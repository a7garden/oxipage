//! Shared deploy pipeline for oxipage — used by the `oxipage deploy` CLI and
//! the console's `POST /deploy` + SSE stream.
//!
//! GitHub Pages deployment is repository-scoped: every git command runs with
//! `Command::current_dir(repo_dir)`, the worktree is a fresh UUID under the
//! system temp dir, cleanup is RAII, and no `bash -c` / external `cp` / `rm`
//! is used. Target values are validated before any command runs.

use oxipage_core::build_manifest::BuildManifest;
use oxipage_core::site_paths::GitHubPagesTarget;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tokio::sync::mpsc;

/// Result of a deploy run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeployOutcome {
    Deployed {
        url: String,
        commit: String,
    },
    Unchanged {
        url: String,
        commit: String,
    },
}

/// Errors from a deploy run.
#[derive(Debug, thiserror::Error)]
pub enum DeployError {
    #[error("build output missing")]
    OutDirMissing,
    #[error("manifest base mismatch: expected {expected}, got {actual}")]
    ManifestBaseMismatch { expected: String, actual: String },
    #[error("invalid target: {0}")]
    InvalidTarget(String),
    #[error("gh not installed")]
    GhNotFound,
    #[error("gh authentication required")]
    NotAuthenticated,
    #[error("not a git repository")]
    NotGitRepository,
    #[error("origin mismatch")]
    OriginMismatch,
    #[error("git failed during {0}")]
    Git(&'static str),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Progress events for a deploy run. Serialized as
/// `{"event":"<variant>", ...}` for the console SSE stream.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case", tag = "event")]
pub enum DeployEvent {
    PreflightStarted,
    GhReady,
    AuthReady,
    RepositoryReady,
    WorktreeReady,
    FilesCopied { count: usize },
    CommitCreated { commit: String },
    Pushing { branch: String },
    Deployed { url: String, commit: String },
    Unchanged { url: String, commit: String },
    Failed { code: String, error: String },
}

/// Whether a git remote URL points at the configured target repository.
/// Accepts https, ssh git@, and ssh:// forms, with or without a trailing `.git`.
pub fn origin_matches(remote: &str, target: &GitHubPagesTarget) -> bool {
    let r = remote.trim().trim_end_matches('/').trim_end_matches(".git");
    r == format!("https://github.com/{}/{}", target.owner, target.repo)
        || r == format!("git@github.com:{}/{}", target.owner, target.repo)
        || r == format!("ssh://git@github.com/{}/{}", target.owner, target.repo)
}

fn copy_tree(src: &Path, dst: &Path) -> std::io::Result<usize> {
    let mut n = 0;
    for e in std::fs::read_dir(src)? {
        let e = e?;
        let from = e.path();
        let to = dst.join(e.file_name());
        if from.is_dir() {
            std::fs::create_dir_all(&to)?;
            n += copy_tree(&from, &to)?;
        } else {
            std::fs::copy(from, to)?;
            n += 1;
        }
    }
    Ok(n)
}

/// Remove every entry in `dir` except the `.git` directory (the worktree's
/// own git metadata must survive).
fn clear(dir: &Path) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.file_name().is_some_and(|n| n == ".git") {
            continue;
        }
        if p.is_dir() {
            std::fs::remove_dir_all(p)?;
        } else {
            std::fs::remove_file(p)?;
        }
    }
    Ok(())
}

/// RAII cleanup: removes the worktree entry + prunes + deletes the temp dir
/// even if the deploy fails partway through.
struct Cleanup {
    repo: PathBuf,
    worktree: PathBuf,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        let _ = Command::new("git")
            .current_dir(&self.repo)
            .args(["worktree", "remove", "--force"])
            .arg(&self.worktree)
            .output();
        let _ = Command::new("git")
            .current_dir(&self.repo)
            .args(["worktree", "prune"])
            .output();
        let _ = std::fs::remove_dir_all(&self.worktree);
    }
}

/// Run a subprocess with an explicit CWD and argument list (no shell
/// interpolation). A missing `gh` binary maps to [`DeployError::GhNotFound`].
fn run(cwd: &Path, program: &str, args: &[&str], step: &'static str) -> Result<Output, DeployError> {
    let o = Command::new(program)
        .current_dir(cwd)
        .args(args)
        .output()
        .map_err(|e| {
            if program == "gh" && e.kind() == std::io::ErrorKind::NotFound {
                DeployError::GhNotFound
            } else {
                DeployError::Io(e)
            }
        })?;
    if o.status.success() {
        Ok(o)
    } else {
        Err(DeployError::Git(step))
    }
}

/// Deploy `out_dir` to GitHub Pages for the configured target.
///
/// Blocking — runs git/gh subprocesses and emits progress via `tx` (mpsc,
/// `blocking_send`). Call from `spawn_blocking` (console) or a drain task
/// (CLI). Every git command is scoped to `repo_dir`; the worktree is a fresh
/// UUID under the system temp dir.
pub fn deploy_github_pages(
    repo_dir: &Path,
    out_dir: &Path,
    target: &GitHubPagesTarget,
    manifest: &BuildManifest,
    tx: &mpsc::Sender<DeployEvent>,
) -> Result<DeployOutcome, DeployError> {
    target
        .validate()
        .map_err(|e| DeployError::InvalidTarget(e.to_string()))?;
    if !out_dir.join("index.html").is_file() {
        return Err(DeployError::OutDirMissing);
    }
    let expected = target.base_path();
    if manifest.deployment_base != expected {
        return Err(DeployError::ManifestBaseMismatch {
            expected,
            actual: manifest.deployment_base.clone(),
        });
    }
    let _ = tx.blocking_send(DeployEvent::PreflightStarted);

    run(repo_dir, "gh", &["--version"], "gh version")?;
    let _ = tx.blocking_send(DeployEvent::GhReady);
    if run(repo_dir, "gh", &["auth", "status"], "gh auth").is_err() {
        return Err(DeployError::NotAuthenticated);
    }
    let _ = tx.blocking_send(DeployEvent::AuthReady);

    run(repo_dir, "git", &["rev-parse", "--is-inside-work-tree"], "repository")
        .map_err(|_| DeployError::NotGitRepository)?;
    let remote = run(repo_dir, "git", &["remote", "get-url", "origin"], "origin")?;
    if !origin_matches(&String::from_utf8_lossy(&remote.stdout), target) {
        return Err(DeployError::OriginMismatch);
    }
    let _ = tx.blocking_send(DeployEvent::RepositoryReady);

    let work = std::env::temp_dir().join(format!("oxipage-deploy-{}", uuid::Uuid::new_v4()));
    let w = work.to_string_lossy().into_owned();
    let remote_ref = format!("refs/remotes/origin/{}", target.branch);
    let exists = Command::new("git")
        .current_dir(repo_dir)
        .args(["show-ref", "--verify", "--quiet", &remote_ref])
        .status()
        .is_ok_and(|s| s.success());
    if exists {
        run(repo_dir, "git", &["worktree", "add", "--detach", &w, &remote_ref], "worktree")?;
    } else {
        run(repo_dir, "git", &["worktree", "add", "--detach", &w], "worktree")?;
    }
    let cleanup = Cleanup {
        repo: repo_dir.into(),
        worktree: work.clone(),
    };
    let _ = tx.blocking_send(DeployEvent::WorktreeReady);

    clear(&work)?;
    let count = copy_tree(out_dir, &work)?;
    let _ = tx.blocking_send(DeployEvent::FilesCopied { count });
    run(&work, "git", &["add", "-A"], "add")?;
    let changed = !Command::new("git")
        .current_dir(&work)
        .args(["diff", "--cached", "--quiet"])
        .status()?
        .success();
    let url = target.pages_url();
    if !changed {
        let o = run(&work, "git", &["rev-parse", "HEAD"], "head")?;
        let commit = String::from_utf8_lossy(&o.stdout).trim().to_string();
        let result = DeployOutcome::Unchanged {
            url: url.clone(),
            commit: commit.clone(),
        };
        let _ = tx.blocking_send(DeployEvent::Unchanged { url, commit });
        drop(cleanup);
        return Ok(result);
    }

    let msg = format!("deploy: {}", manifest.build_id);
    run(&work, "git", &["commit", "-m", &msg], "commit")?;
    let o = run(&work, "git", &["rev-parse", "HEAD"], "head")?;
    let commit = String::from_utf8_lossy(&o.stdout).trim().to_string();
    let _ = tx.blocking_send(DeployEvent::CommitCreated {
        commit: commit.clone(),
    });
    let push = format!("HEAD:refs/heads/{}", target.branch);
    let _ = tx.blocking_send(DeployEvent::Pushing {
        branch: target.branch.clone(),
    });
    run(&work, "git", &["push", "origin", &push], "push")?;
    let result = DeployOutcome::Deployed {
        url: url.clone(),
        commit: commit.clone(),
    };
    let _ = tx.blocking_send(DeployEvent::Deployed { url, commit });
    drop(cleanup);
    Ok(result)
}
