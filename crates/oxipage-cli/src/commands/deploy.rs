use crate::output::Output;
use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
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

pub(crate) async fn deploy(c: DeployArgs, out: &Output) -> anyhow::Result<()> {
    let data_dir = resolve_data_dir()?;
    let out_dir = data_dir.join("out");

    if c.dry_run {
        let _ = out.ok(format!(
            "dry-run: would deploy {} to GitHub Pages",
            out_dir.display()
        ));
        return Ok(());
    }

    match c.target.as_str() {
        "github-pages" => {
            let (tx, mut rx) = tokio::sync::mpsc::channel::<oxipage_deploy::DeployEvent>(32);
            let out_dir_owned = out_dir.clone();
            let handle = tokio::task::spawn_blocking(move || {
                oxipage_deploy::deploy_github_pages(&out_dir_owned, &tx)
            });
            while let Some(ev) = rx.recv().await {
                let _ = out.ok(deploy_event_label(&ev));
            }
            match handle.await {
                Ok(Ok(())) => Ok(()),
                Ok(Err(e)) => Err(e.into()),
                Err(e) => Err(anyhow::anyhow!("deploy task panicked: {e}")),
            }
        }
        "cloudflare" | "netlify" => {
            anyhow::bail!("{} target not yet implemented", c.target);
        }
        _ => anyhow::bail!("unsupported deploy target: {}", c.target),
    }
}

fn deploy_event_label(ev: &oxipage_deploy::DeployEvent) -> String {
    use oxipage_deploy::DeployEvent;
    match ev {
        DeployEvent::GhCheck => "Checking GitHub CLI…".into(),
        DeployEvent::AuthCheck => "Verifying authentication…".into(),
        DeployEvent::WorktreeReady => "Prepared gh-pages worktree".into(),
        DeployEvent::FilesCopied { count } => format!("Copied {count} files to worktree"),
        DeployEvent::Pushing => "Pushing to gh-pages…".into(),
        DeployEvent::Deployed { url } => format!("Deployed to GitHub Pages: {url}"),
        DeployEvent::Failed { error } => format!("Deploy failed: {error}"),
    }
}

fn resolve_data_dir() -> anyhow::Result<PathBuf> {
    let config_path = std::env::var("OXIPAGE_CONFIG")
        .map(PathBuf::from)
        .ok()
        .filter(|p| p.exists());

    if let Some(ref path) = config_path {
        let toml_str = std::fs::read_to_string(path)?;
        let value: toml::Value = toml::from_str(&toml_str)?;
        if let Some(data_dir) = value
            .get("server")
            .and_then(|s| s.get("data_dir"))
            .and_then(|d| d.as_str())
        {
            return Ok(PathBuf::from(data_dir));
        }
    }

    if let Ok(dir) = std::env::var("OXIPAGE_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }

    Ok(PathBuf::from("data"))
}
