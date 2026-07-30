//! Shared deploy pipeline for oxipage — used by the `oxipage deploy` CLI and
//! the console's `POST /deploy` + SSE stream.
//!
//! Currently implements GitHub Pages via a `git worktree` + `gh-pages` branch
//! push. Deploy steps are emitted as [`DeployEvent`] through an mpsc channel so
//! both callers can surface progress (CLI prints; console streams as SSE).

use std::path::Path;
use std::process::Command;
use tokio::sync::mpsc;

/// Errors from a deploy run.
#[derive(Debug, thiserror::Error)]
pub enum DeployError {
    #[error("out directory not found at {0}. Run `oxipage build` first.")]
    OutDirMissing(String),
    #[error("gh CLI not found. Install it from https://cli.github.com/")]
    GhNotFound,
    #[error("gh CLI not available")]
    GhUnavailable,
    #[error("Not authenticated with GitHub CLI. Run `gh auth login` first.")]
    NotAuthenticated,
    #[error("Failed to create gh-pages worktree")]
    WorktreeCreateFailed,
    #[error("Failed to checkout gh-pages worktree")]
    WorktreeCheckoutFailed,
    #[error("Failed to copy build output to worktree")]
    CopyFailed,
    #[error("GitHub Pages deploy failed: {0}")]
    PushFailed(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Progress events for a deploy run. Emitted through an mpsc channel;
/// serialized as `{"event":"<variant>", ...}` for the console SSE stream.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "event")]
pub enum DeployEvent {
    GhCheck,
    AuthCheck,
    WorktreeReady,
    FilesCopied { count: usize },
    Pushing,
    Deployed { url: String },
    Failed { error: String },
}

/// Deploy `out_dir` to GitHub Pages.
///
/// Blocking — runs git/gh subprocesses and emits progress via `tx` (mpsc,
/// `blocking_send`). Call from `spawn_blocking` (console) or a drain task
/// (CLI). Assumes the process CWD is the site's git repository (the worktree
/// is created relative to the current `.git`).
pub fn deploy_github_pages(
    out_dir: &Path,
    tx: &mpsc::Sender<DeployEvent>,
) -> Result<(), DeployError> {
    if !out_dir.exists() {
        return Err(DeployError::OutDirMissing(out_dir.display().to_string()));
    }

    // gh CLI availability
    let _ = tx.blocking_send(DeployEvent::GhCheck);
    let gh_check = Command::new("gh")
        .arg("--version")
        .output()
        .map_err(|_| DeployError::GhNotFound)?;
    if !gh_check.status.success() {
        return Err(DeployError::GhUnavailable);
    }

    // gh auth status
    let _ = tx.blocking_send(DeployEvent::AuthCheck);
    let auth_check = Command::new("gh")
        .args(["auth", "status"])
        .output()
        .map_err(|_| DeployError::NotAuthenticated)?;
    if !auth_check.status.success() {
        return Err(DeployError::NotAuthenticated);
    }

    let worktree_dir = format!("/tmp/oxipage-deploy-{}", std::process::id());

    // Does a gh-pages branch already exist?
    let has_gh_pages = Command::new("git")
        .args([
            "--git-dir",
            ".git",
            "rev-parse",
            "--verify",
            "refs/heads/gh-pages",
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_gh_pages {
        // Create an orphan gh-pages branch via a detached worktree.
        let status = Command::new("git")
            .args(["worktree", "add", "--detach", &worktree_dir])
            .output()?;
        if !status.status.success() {
            return Err(DeployError::WorktreeCreateFailed);
        }
        Command::new("git")
            .args([
                "--git-dir",
                &format!("{worktree_dir}/.git"),
                "symbolic-ref",
                "HEAD",
                "refs/heads/gh-pages",
            ])
            .output()?;
        let _ = Command::new("rm")
            .args([
                "-rf",
                &format!("{worktree_dir}/*"),
                &format!("{worktree_dir}/.*"),
            ])
            .output();
    } else {
        let status = Command::new("git")
            .args(["worktree", "add", &worktree_dir, "gh-pages"])
            .output()?;
        if !status.status.success() {
            return Err(DeployError::WorktreeCheckoutFailed);
        }
    }
    let _ = tx.blocking_send(DeployEvent::WorktreeReady);

    // Copy built output into the worktree.
    let count = count_files(out_dir);
    let copy_status = Command::new("cp")
        .args(["-rf", &format!("{}/.", out_dir.display()), &worktree_dir])
        .output()?;
    if !copy_status.status.success() {
        return Err(DeployError::CopyFailed);
    }
    let _ = tx.blocking_send(DeployEvent::FilesCopied { count });

    // Commit + push.
    let _ = tx.blocking_send(DeployEvent::Pushing);
    let url = remote_url().unwrap_or_else(|| "GitHub Pages".to_string());
    let commit_status = Command::new("bash")
        .args([
            "-c",
            &format!(
                "cd {worktree_dir} && git add -A && git commit -m 'deploy: {}' && git push origin gh-pages",
                timestamp()
            ),
        ])
        .output()?;

    // Clean up the worktree regardless of outcome.
    let _ = Command::new("git")
        .args(["worktree", "remove", &worktree_dir])
        .output();

    if commit_status.status.success() {
        let _ = tx.blocking_send(DeployEvent::Deployed { url });
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&commit_status.stderr).to_string();
        let _ = tx.blocking_send(DeployEvent::Failed {
            error: stderr.clone(),
        });
        Err(DeployError::PushFailed(stderr))
    }
}

fn count_files(path: &Path) -> usize {
    let mut n = 0;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_file() {
                n += 1;
            } else if p.is_dir() {
                n += count_files(&p);
            }
        }
    }
    n
}

fn remote_url() -> Option<String> {
    Command::new("git")
        .args(["config", "--get", "remote.origin.url"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_else(|_| "unknown".into())
}
