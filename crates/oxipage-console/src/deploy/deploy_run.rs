//! Deploy-run infrastructure — mirrors `build_run.rs` for deploys.
//!
//! `DeployGuard` is a registry-level singleton (one `DashMap`) tracking the
//! single in-flight deploy per site slug, separate from the build guard so a
//! build and a deploy may run concurrently. `DeployRun` holds the out-dir and a
//! broadcast channel for SSE subscribers.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use dashmap::DashMap;
use oxipage_core::build_manifest::BuildManifest;
use oxipage_core::site_paths::GitHubPagesTarget;
use oxipage_deploy::DeployEvent;
use tokio::sync::broadcast;

/// Everything a lazy-started deploy task needs, independent of the HTTP request
/// that triggered it. Stored in [`DeployGuard`] keyed by site slug.
pub struct DeployRun {
    pub id: String,
    pub tx: broadcast::Sender<DeployEvent>,
    pub started: AtomicBool,
    pub repo_dir: PathBuf,
    pub out_dir: PathBuf,
    pub target: GitHubPagesTarget,
    pub manifest: BuildManifest,
    pub slug: String,
}

/// Registry-level singleton tracking the single in-flight deploy per site.
pub struct DeployGuard {
    runs: DashMap<String, DeployRun>,
}

impl DeployGuard {
    pub fn new() -> Self {
        Self {
            runs: DashMap::new(),
        }
    }

    /// Reserve a deploy slot for `slug`. Returns `Err(existing_deploy_id)` if a
    /// deploy is already in flight (→ HTTP 409 Conflict).
    pub fn try_start(&self, slug: &str, run: DeployRun) -> Result<(), String> {
        use dashmap::mapref::entry::Entry;
        match self.runs.entry(slug.to_string()) {
            Entry::Occupied(o) => Err(o.get().id.clone()),
            Entry::Vacant(v) => {
                v.insert(run);
                Ok(())
            }
        }
    }

    /// Look up an in-flight deploy by slug, returning a new broadcast receiver
    /// (one SSE subscriber) plus the deploy id.
    pub fn subscribe(&self, slug: &str) -> Option<(String, broadcast::Receiver<DeployEvent>)> {
        self.runs.get(slug).map(|r| (r.id.clone(), r.tx.subscribe()))
    }

    /// Release the deploy slot for `slug`.
    pub fn finish(&self, slug: &str) {
        self.runs.remove(slug);
    }
}

impl Default for DeployGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl DeployGuard {
    /// Start the in-flight deploy for `slug` exactly once (AtomicBool CAS).
    /// The first caller — an SSE subscriber or the 3s watchdog — wins and
    /// spawns the deploy; later callers are no-ops. `deploy_github_pages`
    /// emits its own terminal `Deployed`/`Failed` event, so here we only wait
    /// for completion and release the slot. Returns `false` if no deploy is
    /// registered for `slug`.
    ///
    /// Must be called from a Tokio runtime context (captures `Handle::current()`).
    pub fn ensure_deploy_started(self: &Arc<Self>, slug: &str) -> bool {
        use std::sync::atomic::Ordering;

        let Some(run) = self.runs.get(slug) else {
            return false;
        };
        if run
            .started
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return true;
        }

        let repo_dir = run.repo_dir.clone();
        let out_dir = run.out_dir.clone();
        let target = run.target.clone();
        let manifest = run.manifest.clone();
        let bcast = run.tx.clone();
        let slug_owned = slug.to_string();
        drop(run);

        let (mpsc_tx, mut mpsc_rx) = tokio::sync::mpsc::channel::<DeployEvent>(32);

        // Relay mpsc (sync, from the deploy task) → broadcast (async, to SSE).
        let relay_bcast = bcast.clone();
        tokio::spawn(async move {
            while let Some(ev) = mpsc_rx.recv().await {
                let _ = relay_bcast.send(ev);
            }
        });

        let guard = self.clone();
        tokio::spawn(async move {
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
                .map_err(|e| oxipage_deploy::DeployError::Io(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("deploy task panicked: {e}"),
                )))
                .and_then(|r| r);
            if let Err(e) = &outcome {
                let _ = bcast.send(DeployEvent::Failed {
                    code: "deploy_failed".into(),
                    error: e.to_string(),
                });
            }
            // deploy_github_pages emits the terminal Deployed/Unchanged event
            // itself; just release the slot so the slug isn't stuck.
            guard.finish(&slug_owned);
        });
        true
    }
}
