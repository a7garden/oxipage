use crate::client::Client;
use crate::output::Output;
use clap::Args;
use std::path::Path;

#[derive(Args, Debug)]
pub struct DeployArgs {
    /// Target platform: github-pages, cloudflare, netlify
    #[arg(long, default_value = "github-pages")]
    pub target: String,
    /// Optional site name (from sites.toml)
    #[arg(long)]
    pub site: Option<String>,
    /// Dry run — shows what would be deployed without actually pushing
    #[arg(long)]
    pub dry_run: bool,
}

pub(crate) async fn deploy(
    c: DeployArgs,
    out: &Output,
    client: &Client,
) -> anyhow::Result<()> {
    // Resolve out directory via server status
    let status = client.get("/api/v1/status").await?;
    let data_dir = status["data_dir"].as_str().unwrap_or("data");
    let out_dir = Path::new(data_dir).join("out");

    match c.target.as_str() {
        "github-pages" => deploy_github_pages(&out_dir, c.dry_run, out).await,
        "cloudflare" | "netlify" => {
            anyhow::bail!("{} target not yet implemented", c.target);
        }
        _ => anyhow::bail!("unsupported deploy target: {}", c.target),
    }
}

async fn deploy_github_pages(
    out_dir: &Path,
    dry_run: bool,
    out: &Output,
) -> anyhow::Result<()> {
    if !out_dir.exists() {
        anyhow::bail!("out directory not found at {}. Run `oxipage build` first.", out_dir.display());
    }

    if dry_run {
        out.ok(format!("dry-run: would deploy {} to GitHub Pages", out_dir.display()));
        return Ok(());
    }

    // Check gh CLI is available
    let gh_check = std::process::Command::new("gh")
        .arg("--version")
        .output()
        .map_err(|_| anyhow::anyhow!("gh CLI not found. Install it from https://cli.github.com/"))?;
    if !gh_check.status.success() {
        anyhow::bail!("gh CLI not available");
    }

    // Check auth
    let auth_check = std::process::Command::new("gh")
        .args(["auth", "status"])
        .output()
        .map_err(|_| anyhow::anyhow!("gh auth check failed"))?;
    if !auth_check.status.success() {
        anyhow::bail!("Not authenticated with GitHub CLI. Run `gh auth login` first.");
    }

    // Get the remote URL for gh-pages branch
    let worktree_dir = format!("/tmp/oxipage-deploy-{}", std::process::id());

    // Create or checkout gh-pages branch using git worktree
    let has_gh_pages = std::process::Command::new("git")
        .args(["--git-dir", ".git", "rev-parse", "--verify", "refs/heads/gh-pages"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !has_gh_pages {
        // Create orphan gh-pages branch
        let status = std::process::Command::new("git")
            .args(["worktree", "add", "--detach", &worktree_dir])
            .output()?;
        if !status.status.success() {
            anyhow::bail!("Failed to create worktree");
        }
        // Init empty branch
        std::process::Command::new("git")
            .args(["--git-dir", &format!("{}/.git", worktree_dir), "symbolic-ref", "HEAD", "refs/heads/gh-pages"])
            .output()?;
        // Remove all files
        std::process::Command::new("rm")
            .arg("-rf")
            .arg(format!("{}/*", worktree_dir))
            .arg(format!("{}/.*", worktree_dir))
            .output()
            .ok();
    } else {
        let status = std::process::Command::new("git")
            .args(["worktree", "add", &worktree_dir, "gh-pages"])
            .output()?;
        if !status.status.success() {
            anyhow::bail!("Failed to checkout gh-pages worktree");
        }
    }

    // Copy built files to worktree
    let copy_status = std::process::Command::new("cp")
        .args(["-rf", &format!("{}/.", out_dir.display()), &worktree_dir])
        .output()?;
    if !copy_status.status.success() {
        anyhow::bail!("Failed to copy build output to worktree");
    }

    // Commit and push
    let commit_status = std::process::Command::new("bash")
        .args([
            "-c",
            &format!(
                "cd {} && git add -A && git commit -m 'deploy: {}' && git push origin gh-pages",
                worktree_dir,
                chrono_now()
            ),
        ])
        .output()?;

    // Clean up worktree
    std::process::Command::new("git")
        .args(["worktree", "remove", &worktree_dir])
        .output()
        .ok();

    if commit_status.status.success() {
        out.ok("Deployed to GitHub Pages");
    } else {
        let stderr = String::from_utf8_lossy(&commit_status.stderr);
        anyhow::bail!("GitHub Pages deploy failed: {}", stderr);
    }

    Ok(())
}

fn chrono_now() -> String {
    // Fallback: use std time if chrono not available
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| format!("{}", d.as_secs()))
        .unwrap_or_else(|_| "unknown".into())
}
