//! Deploy-run orchestration on top of the shared [`SiteOperationGuard`].
//!
//! Mirrors `build_run.rs`: claim the guard's slot, spawn the blocking
//! repository-scoped deploy, relay every [`DeployEvent`] as an
//! [`OperationEvent`], publish a terminal event, and release the slot.

use crate::operations::{OperationEvent, SiteOperationGuard};
use crate::sites_runtime::SiteContext;
use oxipage_core::site_paths::GitHubPagesTarget;
use oxipage_deploy::DeployEvent;
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
    let guard = guard.clone();

    tokio::spawn(async move {
        let (mpsc_tx, mut mpsc_rx) = tokio::sync::mpsc::channel::<DeployEvent>(32);

        // Relay deploy events (sync, from the deploy task) → OperationEvent
        // broadcast (async, to SSE subscribers).
        let relay_slug = slug.clone();
        let relay_guard = guard.clone();
        tokio::spawn(async move {
            while let Some(ev) = mpsc_rx.recv().await {
                let terminal = matches!(ev, DeployEvent::Deployed { .. } | DeployEvent::Unchanged { .. } | DeployEvent::Failed { .. });
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
                    &repo_dir,
                    &out_dir,
                    &target,
                    &manifest,
                    &mpsc_tx,
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

        // Terminal snapshot + release the slot.
        match &outcome {
            Ok(oxipage_deploy::DeployOutcome::Deployed { url, commit }) => {
                let _ = guard.publish(
                    &slug,
                    OperationEvent::terminal(
                        "deployed",
                        serde_json::json!({ "status": "deployed", "url": url, "commit": commit }),
                    ),
                );
            }
            Ok(oxipage_deploy::DeployOutcome::Unchanged { url, commit }) => {
                let _ = guard.publish(
                    &slug,
                    OperationEvent::terminal(
                        "unchanged",
                        serde_json::json!({ "status": "unchanged", "url": url, "commit": commit }),
                    ),
                );
            }
            Err(e) => {
                let _ = guard.publish(
                    &slug,
                    OperationEvent::terminal(
                        "failed",
                        serde_json::json!({ "status": "failed", "error": e.to_string() }),
                    ),
                );
            }
        }
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
