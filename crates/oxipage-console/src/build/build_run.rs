//! Build-run infrastructure: per-site build guard + in-flight build state.
//!
//! `BuildGuard` is a registry-level singleton (one `DashMap`) tracking the
//! single in-flight build per site slug. `POST /build` calls `try_start`; a
//! concurrent build for the same slug returns 409. The guard is released on
//! every exit path when the build finishes (RAII via the orchestration task).
//!
//! `BuildRun` holds everything the lazy-started build task needs (db, builders,
//! out/media dirs, slug) so the SSE subscriber can run the build without the
//! original request context.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use dashmap::DashMap;
use oxipage_core::build::BuildEvent;
use oxipage_core::builder::BuildExt;
use sqlx::SqlitePool;
use tokio::sync::broadcast;

/// Everything a lazy-started build task needs, independent of the HTTP request
/// that triggered it. Stored in [`BuildGuard`] keyed by site slug.
pub struct BuildRun {
    pub id: String,
    /// Broadcast fan-out for SSE subscribers. Build events are produced once
    /// (into an mpsc inside the build task) and relayed here so any number of
    /// subscribers see the full stream.
    pub tx: broadcast::Sender<BuildEvent>,
    /// CAS guard: the first caller (SSE subscriber or 3s watchdog) to flip
    /// this false→true owns starting the build. Avoids the event-loss race
    /// where the build emits before any subscriber connects.
    pub started: AtomicBool,
    pub started_at: String,
    pub db: SqlitePool,
    pub builders: Arc<Vec<Box<dyn BuildExt>>>,
    pub out_dir: PathBuf,
    pub media_dir: PathBuf,
    pub slug: String,
    /// Site base URL from settings — drives `deployment_base` at write time.
    pub site_base_url: String,
    /// Theme id active at build start.
    pub theme_id: String,
}

/// Registry-level singleton tracking the single in-flight build per site.
pub struct BuildGuard {
    runs: DashMap<String, BuildRun>,
}

impl BuildGuard {
    pub fn new() -> Self {
        Self {
            runs: DashMap::new(),
        }
    }

    /// Reserve a build slot for `slug`. Returns `Err(existing_build_id)` if a
    /// build is already in flight for that slug (→ HTTP 409 Conflict).
    pub fn try_start(&self, slug: &str, run: BuildRun) -> Result<(), String> {
        use dashmap::mapref::entry::Entry;
        match self.runs.entry(slug.to_string()) {
            Entry::Occupied(o) => Err(o.get().id.clone()),
            Entry::Vacant(v) => {
                v.insert(run);
                Ok(())
            }
        }
    }

    /// Look up an in-flight build by slug, returning a new broadcast receiver
    /// (one SSE subscriber) plus the build id.
    pub fn subscribe(&self, slug: &str) -> Option<(String, broadcast::Receiver<BuildEvent>)> {
        self.runs.get(slug).map(|r| (r.id.clone(), r.tx.subscribe()))
    }

    /// Release the build slot for `slug`. Called on every build exit path so a
    /// finished/failed build never permanently blocks the slug.
    pub fn finish(&self, slug: &str) {
        self.runs.remove(slug);
    }
}

impl Default for BuildGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildGuard {
    /// Start the in-flight build for `slug` exactly once (AtomicBool CAS).
    /// The first caller — an SSE subscriber or the 3s watchdog — wins and
    /// spawns the build; later callers are no-ops. Returns `false` if no build
    /// is registered for `slug`.
    ///
    /// Must be called from a Tokio runtime context: it captures
    /// `Handle::current()` for the `spawn_blocking` build and spawns the
    /// relay/recorder tasks.
    pub fn ensure_build_started(self: &Arc<Self>, slug: &str) -> bool {
        use std::sync::atomic::Ordering;

        let Some(run) = self.runs.get(slug) else {
            return false;
        };
        if run
            .started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return true; // already started by another caller
        }

        // Won the CAS — clone everything the build task needs, then release the
        // DashMap read guard before spawning.
        let db = run.db.clone();
        let builders = run.builders.clone();
        let out_dir = run.out_dir.clone();
        let media_dir = run.media_dir.clone();
        let started_at = run.started_at.clone();
        let bcast = run.tx.clone();
        let slug_owned = slug.to_string();
        let site_base_url = run.site_base_url.clone();
        let theme_id = run.theme_id.clone();
        drop(run);

        let rt = tokio::runtime::Handle::current();
        let (mpsc_tx, mut mpsc_rx) = tokio::sync::mpsc::channel::<BuildEvent>(64);

        // Relay mpsc (sync, from the build task) → broadcast (async, to SSE).
        tokio::spawn(async move {
            while let Some(ev) = mpsc_rx.recv().await {
                let _ = bcast.send(ev);
            }
        });

        let guard = self.clone();
        let db_for_log = db.clone();
        let out_dir_for_log = out_dir.clone();
        tokio::spawn(async move {
            let outcome: Result<usize, String> = match tokio::task::spawn_blocking(move || {
                match oxipage_core::build::build_site_with_progress(&db, &builders, &rt, &mpsc_tx)
                {
                    Ok(output) => {
                        let inputs = oxipage_core::builder::BuildInputs::new(
                            &site_base_url,
                            &theme_id,
                            "oxipage",
                        );
                        if let Err(e) = oxipage_core::build_writer::write_build_output(
                            &output,
                            &out_dir,
                            &media_dir,
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
            };

            record_build_log(&db_for_log, &started_at, &out_dir_for_log, &outcome).await;
            guard.finish(&slug_owned);
        });
        true
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
