//! Build-run orchestration on top of the shared [`SiteOperationGuard`].
//!
//! The guard owns the per-site operation slot (conflict detection, broadcast
//! fan-out, retained terminal state) and the start-CAS. This module provides
//! the lazy-start entry point that turns the guard's slot into an actual SSG
//! build: claim the slot, spawn the blocking build, relay every [`BuildEvent`]
//! as an [`OperationEvent`], publish a terminal event, record the log row,
//! and release the slot.

use crate::operations::{OperationEvent, SiteOperationGuard, SiteOperationKind};
use crate::sites_runtime::SiteContext;
use oxibuilder_core::build::BuildEvent;
use oxibuilder_core::builder::BuildInputs;
use sqlx::SqlitePool;
use std::sync::Arc;

/// Start the in-flight build for `slug` exactly once (CAS via the guard).
/// The first caller — an SSE subscriber or the 3s watchdog — wins and spawns
/// the build; later callers are no-ops. Returns `false` if no build is
/// registered for `slug` or another caller already owns starting it.
///
/// Must be called from a Tokio runtime context (captures `Handle::current()`
/// for the `spawn_blocking` build).
pub async fn ensure_build_started(
    guard: &Arc<SiteOperationGuard>,
    ctx: &Arc<SiteContext>,
    build_id: &str,
) -> bool {
    // Only the registered run may be started.
    let Some(snapshot) = guard.current(&ctx.slug) else {
        return false;
    };
    if snapshot.run_id != build_id {
        return false;
    }
    if !guard.try_claim(&ctx.slug) {
        return true; // already started by another caller
    }

    let slug = ctx.slug.clone();
    let db = ctx.db.clone();
    let builders = ctx.builders.clone();
    let out_dir = ctx.out_dir.clone();
    let media_dir = ctx.media_dir.clone();
    let started_at = snapshot.started_at;
    let site_base_url = ctx.settings.read().await.site.base_url.clone();
    let theme_id = oxibuilder_core::theme::active_theme_id(&ctx.db).await;
    let guard = guard.clone();

    tokio::spawn(async move {
        let rt = tokio::runtime::Handle::current();
        let (mpsc_tx, mut mpsc_rx) = tokio::sync::mpsc::channel::<BuildEvent>(64);

        // Relay build events (sync, from the build task) → OperationEvent
        // broadcast (async, to SSE subscribers).
        let relay_slug = slug.clone();
        let relay_guard = guard.clone();
        tokio::spawn(async move {
            while let Some(ev) = mpsc_rx.recv().await {
                let terminal = matches!(
                    ev,
                    BuildEvent::BuildComplete { .. } | BuildEvent::BuildFailed { .. }
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

        let outcome: Result<usize, String> = {
            let db_task = db.clone();
            let out_task = out_dir.clone();
            let media_task = media_dir.clone();
            let builders_task = builders.clone();
            let base_url_task = site_base_url.clone();
            let theme_task = theme_id.clone();
            match tokio::task::spawn_blocking(move || {
                match oxibuilder_core::build::build_site_with_progress(
                    &db_task,
                    &builders_task,
                    &rt,
                    &mpsc_tx,
                ) {
                    Ok(output) => {
                        let inputs = BuildInputs::new(&base_url_task, &theme_task, "oxibuilder");
                        if let Err(e) = oxibuilder_core::build_writer::write_build_output(
                            &output,
                            &out_task,
                            &media_task,
                            &inputs,
                        ) {
                            return Err(format!("write_build_output: {e}"));
                        }
                        Ok(output.pages.len())
                    }
                    Err(e) => Err(e.to_string()),
                }
            })
            .await
            {
                Ok(inner) => inner,
                Err(e) => Err(format!("build task panicked: {e}")),
            }
        };

        record_build_log(&db, &started_at, &out_dir, &outcome).await;

        // Publish a terminal snapshot before releasing the slot so a
        // reconnecting client sees the final state.
        match &outcome {
            Ok(pages) => {
                let _ = guard.publish(
                    &slug,
                    OperationEvent::terminal(
                        "build_complete",
                        serde_json::json!({ "total_pages": pages }),
                    ),
                );
            }
            Err(e) => {
                let _ = guard.publish(
                    &slug,
                    OperationEvent::terminal("build_failed", serde_json::json!({ "error": e })),
                );
            }
        }
        let _ = guard.finish(&slug);
    });
    true
}

fn event_name(ev: &BuildEvent) -> &'static str {
    match ev {
        BuildEvent::BuildStarted { .. } => "build_started",
        BuildEvent::ExtensionStart { .. } => "extension_start",
        BuildEvent::ExtensionDone { .. } => "extension_done",
        BuildEvent::BuildComplete { .. } => "build_complete",
        BuildEvent::BuildFailed { .. } => "build_failed",
    }
}

/// Record a finished build in the per-site `build_log` table. Idempotent schema
/// setup (`finished_at`/`error` columns added if missing — ALTER errors ignored).
async fn record_build_log(
    db: &SqlitePool,
    started_at: &str,
    out_dir: &std::path::Path,
    outcome: &Result<usize, String>,
) {
    let _ = sqlx::query(
        "CREATE TABLE IF NOT EXISTS build_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            status TEXT NOT NULL DEFAULT 'built',
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            page_count INTEGER,
            out_dir TEXT
        )",
    )
    .execute(db)
    .await;
    let _ = sqlx::query("ALTER TABLE build_log ADD COLUMN finished_at TEXT")
        .execute(db)
        .await;
    let _ = sqlx::query("ALTER TABLE build_log ADD COLUMN error TEXT")
        .execute(db)
        .await;

    let (status, page_count, error): (&str, Option<i64>, Option<&str>) = match outcome {
        Ok(n) => ("built", Some(*n as i64), None),
        Err(e) => ("failed", None, Some(e.as_str())),
    };
    let _ = sqlx::query(
        "INSERT INTO build_log (status, page_count, out_dir, created_at, finished_at, error)
         VALUES (?1, ?2, ?3, ?4, datetime('now'), ?5)",
    )
    .bind(status)
    .bind(page_count)
    .bind(out_dir.to_string_lossy().to_string())
    .bind(started_at)
    .bind(error)
    .execute(db)
    .await;
}

/// Site operation kind used when registering a build slot.
pub fn build_operation_kind() -> SiteOperationKind {
    SiteOperationKind::Build
}
