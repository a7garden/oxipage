use crate::client::Client;
use crate::output::Output;
use axum::Router;
use serde_json::json;
use std::net::SocketAddr;
use std::path::Path;
use tokio::net::TcpListener;
use tower_http::services::ServeDir;

const DEFAULT_TOML: &str = r#"[site]
name = "내 Oxipage"
base_url = "http://127.0.0.1:8787"
default_lang = "ko"
languages = ["ko", "en"]

[server]
host = "127.0.0.1"
port = 8787
data_dir = "data"

[extensions]
enabled = ["profile", "blog", "projects", "links"]

[lobby]
default_mode = "grid"
"#;

pub(crate) fn init(out: &Output, config_path: Option<&Path>) -> anyhow::Result<()> {
    let path = config_path
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("oxipage.toml"));
    if path.exists() {
        anyhow::bail!("{} already exists", path.display());
    }
    std::fs::write(&path, DEFAULT_TOML)?;
    out.ok(format!("wrote {}", path.display()))
}

pub(crate) async fn status(out: &Output, client: &Client) -> anyhow::Result<()> {
    let health = client
        .get("/healthz")
        .await
        .map_err(|e| anyhow::anyhow!("healthz failed: {e}"))?;
    let manifest = client
        .get("/api/console/lobby/manifest")
        .await
        .map_err(|e| anyhow::anyhow!("manifest failed: {e}"))?;

    if out.json {
        let summary = json!({
            "endpoint": client.endpoint(),
            "health": health,
            "manifest": manifest,
        });
        println!("{}", serde_json::to_string_pretty(&summary)?);
        return Ok(());
    }

    let ext_count = manifest
        .get("data")
        .and_then(|d| d.get("extensions"))
        .and_then(|e| e.as_array())
        .map(|a| a.len())
        .unwrap_or(0);
    let site_name = manifest
        .get("data")
        .and_then(|d| d.get("site"))
        .and_then(|s| s.get("name"))
        .and_then(|n| n.as_str())
        .unwrap_or("?");
    println!("endpoint:      {}", client.endpoint());
    println!("site:          {site_name}");
    println!("extensions:    {ext_count} enabled");
    Ok(())
}

pub(crate) async fn console(
    port: Option<u16>,
    preview: bool,
    _config_path: Option<&std::path::Path>,
) -> anyhow::Result<()> {
    if preview {
        // Preview mode: serve out/ directory
        let port = port.unwrap_or(8787);
        let out_dir = std::path::PathBuf::from("data/out");
        if !out_dir.exists() {
            anyhow::bail!(
                "out directory not found at {}. Run `oxipage build` first.",
                out_dir.display()
            );
        }
        println!("preview server on http://127.0.0.1:{}", port);
        serve_static_dir(&out_dir, port).await?;
        return Ok(());
    }
    if let Some(p) = port {
        // SAFETY: CLI는 단일 스레드 진입점이며 set_var 직후 run_server가 값을 읽어
        // SocketAddr에 반영한다. 다른 스레드가 환경변수를 경쟁 읽지 않는다.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("OXIPAGE_PORT", p.to_string());
        }
    }
    oxipage_console::run_console().await
}

/// Start a lightweight HTTP server that serves a static directory.
async fn serve_static_dir(dir: &Path, port: u16) -> anyhow::Result<()> {
    let app =
        Router::new().fallback_service(ServeDir::new(dir).append_index_html_on_directories(true));
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
