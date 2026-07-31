//! Deploy-run orchestration on top of the shared [`SiteOperationGuard`].
//!
//! Mirrors `build_run.rs`: claim the guard's slot, spawn the blocking
//! repository-scoped deploy, relay every [`DeployEvent`] as an
//! [`OperationEvent`], publish a terminal event, and release the slot.

use crate::operations::{OperationEvent, SiteOperationGuard};
use crate::sites_runtime::SiteContext;
use oxipage_core::site_paths::GitHubPagesTarget;
use oxipage_deploy::{DeployError, DeployEvent};
use sqlx::SqlitePool;
use std::sync::Arc;

/// Start the in-flight deploy for `slug` exactly once (CAS via the guard).
/// The first caller — an SSE subscriber or the 3s watchdog — wins and spawns
/// the deploy; later callers are no-ops. Returns `false` if no deploy is
/// registered for `slug` or another caller already owns starting it.
///
/// Must be called from a Tokio runtime context.
pub async fn ensure_deploy_started(
    guard: &Arc<SiteOperationGuard>,
    ctx: &Arc<SiteContext>,
    deploy_id: &str,
    target: &GitHubPagesTarget,
) -> bool {
    let Some(snapshot) = guard.current(&ctx.slug) else {
        return false;
    };
    if snapshot.run_id != deploy_id {
        return false;
    }
    if !guard.try_claim(&ctx.slug) {
        return true; // already started by another caller
    }

    let slug = ctx.slug.clone();
    let repo_dir = ctx.project_dir.clone();
    let out_dir = ctx.out_dir.clone();
    let target = target.clone();
    let manifest = match oxipage_core::build_manifest::BuildManifest::read_from(&ctx.out_dir) {
        Ok(Some(m)) => m,
        Ok(None) => {
            let _ = guard.finish(&slug);
            return false;
        }
        Err(_) => {
            let _ = guard.finish(&slug);
            return false;
        }
    };
    let db = ctx.db.clone();
    let started_at = snapshot.started_at;
    let build_id = manifest.build_id.clone();
    let guard = guard.clone();
    let run_id = deploy_id.to_string();

    // Durable 'running' row before the blocking deploy starts.
    record_deploy_start(
        &db,
        &run_id,
        &build_id,
        &manifest.deployment_base,
        &target,
        &started_at,
    )
    .await;

    tokio::spawn(async move {
        let (mpsc_tx, mut mpsc_rx) = tokio::sync::mpsc::channel::<DeployEvent>(32);

        // Relay deploy events (sync, from the deploy task) → OperationEvent
        // broadcast (async, to SSE subscribers).
        let relay_slug = slug.clone();
        let relay_guard = guard.clone();
        tokio::spawn(async move {
            while let Some(ev) = mpsc_rx.recv().await {
                let terminal = matches!(
                    ev,
                    DeployEvent::Deployed { .. }
                        | DeployEvent::Unchanged { .. }
                        | DeployEvent::Failed { .. }
                );
                let _ = relay_guard.publish(
                    &relay_slug,
                    OperationEvent {
                        event: event_name(&ev).to_string(),
                        data: serde_json::to_value(&ev).unwrap_or(serde_json::Value::Null),
                        terminal,
                    },
                );
            }
        });

        let outcome: Result<oxipage_deploy::DeployOutcome, oxipage_deploy::DeployError> =
            tokio::task::spawn_blocking(move || {
                oxipage_deploy::deploy_github_pages(
                    &repo_dir, &out_dir, &target, &manifest, &mpsc_tx,
                )
            })
            .await
            .map_err(|e| {
                oxipage_deploy::DeployError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("deploy task panicked: {e}"),
                ))
            })
            .and_then(|r| r);

        // Terminal DB row + snapshot, then release the slot.
        let (status, url, commit, code, error) = match &outcome {
            Ok(oxipage_deploy::DeployOutcome::Deployed { url, commit }) => (
                "deployed",
                Some(url.clone()),
                Some(commit.clone()),
                None,
                None,
            ),
            Ok(oxipage_deploy::DeployOutcome::Unchanged { url, commit }) => (
                "unchanged",
                Some(url.clone()),
                Some(commit.clone()),
                None,
                None,
            ),
            Err(e) => {
                let (c, m) = normalize(e);
                ("failed", None, None, Some(c), Some(m))
            }
        };
        record_deploy_finish(
            &db,
            &run_id,
            status,
            url.as_deref(),
            commit.as_deref(),
            code,
            error,
        )
        .await;
        let _ = guard.publish(
            &slug,
            OperationEvent::terminal(
                status,
                serde_json::json!({
                    "status": status,
                    "url": url,
                    "commit": commit,
                    "error_code": code,
                    "error": error,
                }),
            ),
        );
        let _ = guard.finish(&slug);
    });
    true
}

fn event_name(ev: &DeployEvent) -> &'static str {
    match ev {
        DeployEvent::PreflightStarted => "preflight_started",
        DeployEvent::GhReady => "gh_ready",
        DeployEvent::AuthReady => "auth_ready",
        DeployEvent::RepositoryReady => "repository_ready",
        DeployEvent::WorktreeReady => "worktree_ready",
        DeployEvent::FilesCopied { .. } => "files_copied",
        DeployEvent::CommitCreated { .. } => "commit_created",
        DeployEvent::Pushing { .. } => "pushing",
        DeployEvent::Deployed { .. } => "deployed",
        DeployEvent::Unchanged { .. } => "unchanged",
        DeployEvent::Failed { .. } => "failed",
    }
}

/// Map a deploy error to a stable machine code + human message (no raw stderr
/// is persisted).
fn normalize(e: &DeployError) -> (&'static str, &'static str) {
    match e {
        DeployError::GhNotFound => ("gh_not_installed", "Install GitHub CLI"),
        DeployError::NotAuthenticated => ("gh_auth_required", "Run gh auth login"),
        DeployError::OriginMismatch => {
            ("origin_mismatch", "Git origin does not match configuration")
        }
        DeployError::ManifestBaseMismatch { .. } => {
            ("stale_build_base", "Rebuild for this Pages base")
        }
        DeployError::OutDirMissing => ("build_required", "Build before deploying"),
        _ => ("deploy_failed", "Deployment failed; inspect the live log"),
    }
}

/// Insert the durable 'running' row. Idempotent schema setup (migration
/// 0007 may not have run in older DBs).
async fn record_deploy_start(
    db: &SqlitePool,
    run_id: &str,
    build_id: &str,
    base_path: &str,
    target: &GitHubPagesTarget,
    started_at: &str,
) {
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS deploy_log(
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            run_id TEXT NOT NULL UNIQUE,
            build_id TEXT NOT NULL,
            target TEXT NOT NULL,
            owner TEXT NOT NULL,
            repo TEXT NOT NULL,
            branch TEXT NOT NULL,
            base_path TEXT NOT NULL,
            status TEXT NOT NULL CHECK(status IN('running','deployed','unchanged','failed')),
            url TEXT,
            commit_sha TEXT,
            error_code TEXT,
            error TEXT,
            started_at TEXT NOT NULL,
            finished_at TEXT
        )",
    )
    .execute(db)
    .await;
    let _ = sqlx::query(
        "INSERT INTO deploy_log
            (run_id, build_id, target, owner, repo, branch, base_path, status, started_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'running', ?8)
         ON CONFLICT(run_id) DO NOTHING",
    )
    .bind(run_id)
    .bind(build_id)
    .bind(target.pages_url())
    .bind(&target.owner)
    .bind(&target.repo)
    .bind(&target.branch)
    .bind(base_path)
    .bind(started_at)
    .execute(db)
    .await;
}

/// Update the terminal state of a deploy row.
async fn record_deploy_finish(
    db: &SqlitePool,
    run_id: &str,
    status: &str,
    url: Option<&str>,
    commit: Option<&str>,
    code: Option<&str>,
    error: Option<&str>,
) {
    let _ = sqlx::query(
        "UPDATE deploy_log SET status=?2, url=?3, commit_sha=?4, error_code=?5, error=?6,
         finished_at=datetime('now') WHERE run_id=?1",
    )
    .bind(run_id)
    .bind(status)
    .bind(url)
    .bind(commit)
    .bind(code)
    .bind(error)
    .execute(db)
    .await;
}
