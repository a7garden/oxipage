use crate::client::Client;
use crate::output::Output;
use clap::Subcommand;
use serde_json::json;

/// Reserved mount path prefixes — mirror of core `RESERVED_MOUNT_PATHS`.
/// Kept here for immediate client-side feedback; the server re-validates.
const RESERVED_PREFIXES: &[&str] = &[
    "assets", "data", "media", "api", "search", "s", "admin", "lobby", "theme",
];

#[derive(Subcommand, Debug, Clone)]
pub enum MountCommand {
    /// 외부 정적 디렉토리를 URL 접두에 마운트로 추가.
    Add {
        #[arg(long)]
        id: String,
        /// Source directory (relative to the config file, or absolute).
        #[arg(long)]
        source: String,
        /// URL prefix / `out/` subdirectory (e.g. `portfolio`).
        #[arg(long)]
        path: String,
        #[arg(long)]
        title_ko: String,
        #[arg(long)]
        title_en: String,
        #[arg(long)]
        desc: Option<String>,
        #[arg(long)]
        icon: Option<String>,
        #[arg(long = "new-tab", default_value_t = false)]
        new_tab: bool,
    },
    /// 설정된 마운트 목록.
    List,
    /// id로 마운트 제거.
    Rm { id: String },
}

pub(crate) async fn mount(c: MountCommand, out: &Output, client: &Client) -> anyhow::Result<()> {
    match c {
        MountCommand::Add {
            id,
            source,
            path,
            title_ko,
            title_en,
            desc,
            icon,
            new_tab,
        } => {
            // Client-side pre-checks for immediate UX; the server is authoritative.
            let top = path.trim_matches('/').split('/').next().unwrap_or("");
            if top.is_empty() {
                anyhow::bail!("path must not be empty");
            }
            if RESERVED_PREFIXES.contains(&top) {
                anyhow::bail!("path uses reserved prefix: {top}");
            }
            // Duplicate-id check against the live list (best-effort; server
            // re-validates atomically via `validate_mounts`).
            let existing = client.get("/api/console/mounts").await?;
            let dup = existing["data"]["mounts"]
                .as_array()
                .is_some_and(|arr| arr.iter().any(|m| m["id"].as_str() == Some(id.as_str())));
            if dup {
                anyhow::bail!("mount id already exists: {id}");
            }

            let payload = json!({
                "id": id,
                "source": source,
                "path": path,
                "title_ko": title_ko,
                "title_en": title_en,
                "description": desc,
                "icon": icon,
                "open_in_new_tab": new_tab,
            });
            let res = client.post("/api/console/mounts", &payload).await?;
            out.data(res, "mount added")
        }
        MountCommand::List => {
            let res = client.get("/api/console/mounts").await?;
            out.data(res, "mounts")
        }
        MountCommand::Rm { id } => {
            let res = client.delete(&format!("/api/console/mounts/{id}")).await?;
            out.data(res, "mount removed")
        }
    }
}
