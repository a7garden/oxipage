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
            // Repository-scoped deploy: project dir from config, target from
            // [deploy.github_pages], manifest from the latest build output.
            let project = super::resolve_project_dir()?;
            let config_path = std::env::var("OXIPAGE_CONFIG")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("oxipage.toml"));
            let cfg = if config_path.exists() {
                oxipage_core::config::Config::load(&config_path)?
            } else {
                oxipage_core::config::Config::default()
            };
            let target = cfg
                .deploy
                .github_pages
                .ok_or_else(|| anyhow::anyhow!("[deploy.github_pages] is not configured"))?;
            target.validate()?;
            let manifest = oxipage_core::build_manifest::BuildManifest::read_from(&out_dir)?
                .ok_or_else(|| anyhow::anyhow!("build required: no manifest in {}", out_dir.display()))?;

            let (tx, mut rx) = tokio::sync::mpsc::channel::<oxipage_deploy::DeployEvent>(32);
            let repo = project.clone();
            let out_dir_owned = out_dir.clone();
            let target2 = target.clone();
            let handle = tokio::task::spawn_blocking(move || {
                oxipage_deploy::deploy_github_pages(&repo, &out_dir_owned, &target2, &manifest, &tx)
            });
            while let Some(ev) = rx.recv().await {
                let _ = out.ok(deploy_event_label(&ev));
            }
            match handle.await {
                Ok(Ok(oxipage_deploy::DeployOutcome::Deployed { url, commit })) => {
                    out.ok(format!("deployed {commit} to {url}"))
                }
                Ok(Ok(oxipage_deploy::DeployOutcome::Unchanged { url, commit })) => {
                    out.ok(format!("unchanged at {commit}: {url}"))
                }
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
        DeployEvent::PreflightStarted => "Preflight checks…".into(),
        DeployEvent::GhReady => "GitHub CLI ready".into(),
        DeployEvent::AuthReady => "Authenticated with GitHub CLI".into(),
        DeployEvent::RepositoryReady => "Repository verified".into(),
        DeployEvent::WorktreeReady => "Prepared gh-pages worktree".into(),
        DeployEvent::FilesCopied { count } => format!("Copied {count} files to worktree"),
        DeployEvent::CommitCreated { commit } => format!("Committed {commit}"),
        DeployEvent::Pushing { branch } => format!("Pushing to {branch}…"),
        DeployEvent::Deployed { url, commit } => {
            format!("Deployed {commit} to GitHub Pages: {url}")
        }
        DeployEvent::Unchanged { url, commit } => {
            format!("Unchanged at {commit}: {url}")
        }
        DeployEvent::Failed { error, .. } => format!("Deploy failed: {error}"),
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
