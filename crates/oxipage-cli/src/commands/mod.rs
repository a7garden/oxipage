//! 서브커맨드 정의 + 디스패치. 서브모듈은 commands/ 디렉토리에 분산.

mod auth;
mod backup;
mod blog;
mod extension;
mod init_status_serve;
mod link;
mod lobby;
mod project;
mod site;

pub use auth::AuthCommand;
pub use backup::BackupCommand;
pub use blog::BlogCommand;
pub use extension::ExtensionCommand;
pub use link::LinkCommand;
pub use lobby::LobbyCommand;
pub use project::ProjectCommand;
pub use site::SiteCommand;

use crate::client::Client;
use crate::credentials;
use crate::output::Output;
use crate::sites;
use crate::{Cli, Command};

// ───────────────────────── dispatch ─────────────────────────

pub async fn dispatch(cli: Cli) -> anyhow::Result<()> {
    let out = Output::new(cli.json);
    let sites_file = sites::SitesFile::load();

    // Resolve active site name from --site flag (or OXIPAGE_SITE, or default_site).
    let site_name = resolve_site_name(cli.site.as_deref(), &sites_file)?;

    // Handle site management commands early — they don't need HTTP client.
    if let Command::Site(c) = &cli.command {
        return site::dispatch_site(c, &out, &sites_file, site_name).await;
    }

    // Resolve endpoint + token, build single Client for all subsequent commands.
    let endpoint = resolve_endpoint(
        cli.endpoint.clone(),
        site_name,
        &sites_file,
        cli.config.as_deref(),
    )?;
    let token = resolve_token(cli.token.clone(), site_name, &sites_file)?;
    let client = Client::new(endpoint, token, cli.insecure)?;

    match cli.command {
        Command::Init => init_status_serve::init(&out, cli.config.as_deref()),
        Command::Status => init_status_serve::status(&out, &client).await,
        Command::Serve { port } => init_status_serve::serve(port, cli.config.as_deref()).await,
        Command::Auth(c) => auth::auth(c, &out, &client).await,
        Command::Blog(c) => blog::blog(c, &out, &client).await,
        Command::Project(c) => project::project(c, &out, &client).await,
        Command::Link(c) => link::link(c, &out, &client).await,
        Command::Lobby(c) => lobby::lobby(c, &out, &client).await,
        Command::Extension(c) => extension::extension(c, &out, &client).await,
        Command::Backup(c) => backup::backup(c, &out, &client).await,
        Command::Site(_) => unreachable!(), // handled above
    }
}

// ──────────────────────── resolution ────────────────────────

/// Resolve active site name: `--site` flag > OXIPAGE_SITE env > default_site > None.
/// Returns `Err` only when `--site` is given but the named site doesn't exist.
fn resolve_site_name<'a>(
    cli_site: Option<&'a str>,
    sites_file: &'a sites::SitesFile,
) -> anyhow::Result<Option<&'a str>> {
    // 1. --site flag (explicit, highest priority)
    if let Some(name) = cli_site {
        if !name.is_empty() {
            if sites_file.exists(name) {
                return Ok(Some(name));
            }
            anyhow::bail!("site '{name}' not found — use `oxipage site add` to create it");
        }
    }
    // 2. OXIPAGE_SITE env — 명시적이므로 flag와 동일하게 존재 여부 검증
    if let Ok(env) = std::env::var("OXIPAGE_SITE") {
        if !env.is_empty() {
            if sites_file.exists(&env) {
                return Ok(sites_file.resolve_name(None));
            }
            anyhow::bail!("site '{env}' (from OXIPAGE_SITE env) not found — use `oxipage site add` to create it");
        }
    }
    // 3. default_site (from sites.toml)
    Ok(sites_file.resolve_name(None))
}

fn resolve_endpoint(
    cli_endpoint: Option<String>,
    site_name: Option<&str>,
    sites_file: &sites::SitesFile,
    config_path: Option<&std::path::Path>,
) -> anyhow::Result<String> {
    // 1. --endpoint flag (explicit override, highest priority)
    if let Some(e) = cli_endpoint
        && !e.is_empty()
    {
        return Ok(e);
    }
    // 2-4. Site resolution (--site, OXIPAGE_SITE env, or default_site)
    if let Some(ep) = sites_file.resolve_endpoint(site_name) {
        return Ok(ep);
    }
    // 5. OXIPAGE_ENDPOINT env (legacy)
    if let Ok(e) = std::env::var("OXIPAGE_ENDPOINT")
        && !e.is_empty()
    {
        return Ok(e);
    }
    // 6. oxipage.toml [site].base_url
    let cfg_path = config_path
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("oxipage.toml"));
    if cfg_path.exists()
        && let Ok(cfg) = oxipage_core::config::Config::load(&cfg_path)
        && !cfg.site.base_url.is_empty()
    {
        return Ok(cfg.site.base_url);
    }
    // 7. Hard-coded fallback
    Ok("http://127.0.0.1:8787".into())
}

fn resolve_token(
    cli_token: Option<String>,
    site_name: Option<&str>,
    sites_file: &sites::SitesFile,
) -> anyhow::Result<Option<String>> {
    // 1. --token flag (explicit override)
    if let Some(t) = cli_token
        && !t.is_empty()
    {
        return Ok(Some(t));
    }
    // 2-4. Site resolution — if resolved site has a token, use it.
    // If site has no token, FALL THROUGH to legacy chain (doc/09 §9.5).
    if let Some(tok) = sites_file.resolve_token(site_name) {
        return Ok(Some(tok));
    }
    // 5. OXIPAGE_TOKEN env (legacy)
    if let Ok(t) = std::env::var("OXIPAGE_TOKEN")
        && !t.is_empty()
    {
        return Ok(Some(t));
    }
    // 6. ~/.config/oxipage/credentials (legacy)
    Ok(credentials::load_token().unwrap_or(None))
}

/// Used by sub-module handlers.
pub(crate) fn require_token(client: &Client) -> anyhow::Result<()> {
    if !client.has_token() {
        anyhow::bail!(
            "no token found — set OXIPAGE_TOKEN env, pass --token, or run `oxipage auth token set <token>`"
        );
    }
    Ok(())
}

// ───────────────────────── unit tests ─────────────────────────
// Env-dependent paths (OXIPAGE_ENDPOINT, OXIPAGE_TOKEN) tested in E2E smoke.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites::SitesFile;

    fn empty_sites() -> SitesFile {
        SitesFile::default()
    }

    fn sites_with_one(name: &str, ep: &str, tok: Option<&str>) -> SitesFile {
        let mut s = SitesFile::default();
        s.sites.insert(
            name.to_string(),
            sites::SiteEntry {
                endpoint: ep.to_string(),
                token: tok.map(String::from),
            },
        );
        s.default_site = Some(name.to_string());
        s
    }

    // ─── resolve_endpoint ───

    #[test]
    fn endpoint_cli_flag_wins() {
        let sites = empty_sites();
        let result = resolve_endpoint(Some("https://cli.example.com".into()), None, &sites, None).unwrap();
        assert_eq!(result, "https://cli.example.com");
    }

    #[test]
    fn endpoint_cli_flag_overrides_site() {
        let sites = sites_with_one("prod", "https://prod.example.com", None);
        let result = resolve_endpoint(Some("https://override.example.com".into()), Some("prod"), &sites, None).unwrap();
        assert_eq!(result, "https://override.example.com");
    }

    #[test]
    fn endpoint_site_resolved() {
        let sites = sites_with_one("prod", "https://prod.example.com", None);
        let result = resolve_endpoint(None, Some("prod"), &sites, None).unwrap();
        assert_eq!(result, "https://prod.example.com");
    }

    #[test]
    fn endpoint_fallback_to_127() {
        let sites = empty_sites();
        let result = resolve_endpoint(None, None, &sites, None).unwrap();
        assert_eq!(result, "http://127.0.0.1:8787");
    }

    // ─── resolve_token ───

    #[test]
    fn token_cli_flag_wins() {
        let sites = empty_sites();
        let result = resolve_token(Some("cli_tok".into()), None, &sites).unwrap();
        assert_eq!(result, Some("cli_tok".into()));
    }

    #[test]
    fn token_site_token_used() {
        let sites = sites_with_one("prod", "https://prod.example.com", Some("site_tok"));
        let result = resolve_token(None, Some("prod"), &sites).unwrap();
        assert_eq!(result, Some("site_tok".into()));
    }

    #[test]
    fn token_site_without_token_returns_none() {
        let sites = sites_with_one("tokenless", "https://example.com", None);
        let result = resolve_token(None, Some("tokenless"), &sites).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn token_none_when_nothing_configured() {
        let sites = empty_sites();
        let result = resolve_token(None, None, &sites).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn token_cli_flag_overrides_site() {
        let sites = sites_with_one("prod", "https://prod.example.com", Some("site_tok"));
        let result = resolve_token(Some("cli_tok".into()), Some("prod"), &sites).unwrap();
        assert_eq!(result, Some("cli_tok".into()));
    }

    // ─── resolve_site_name ───

    #[test]
    fn site_name_flag_valid() {
        let sites = sites_with_one("prod", "url", None);
        let result = resolve_site_name(Some("prod"), &sites).unwrap();
        assert_eq!(result, Some("prod"));
    }

    #[test]
    fn site_name_flag_unknown_errors() {
        let sites = empty_sites();
        let result = resolve_site_name(Some("nope"), &sites);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn site_name_default_site_when_no_flag() {
        let sites = sites_with_one("default-site", "url", None);
        let result = resolve_site_name(None, &sites).unwrap();
        assert_eq!(result, Some("default-site"));
    }

    #[test]
    fn site_name_no_flag_no_sites_returns_none() {
        let sites = empty_sites();
        let result = resolve_site_name(None, &sites).unwrap();
        assert_eq!(result, None);
    }
}
