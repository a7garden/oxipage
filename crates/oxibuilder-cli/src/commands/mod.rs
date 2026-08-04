//! 서브커맨드 정의 + 디스패치. 서브모듈은 commands/ 디렉토리에 분산.

mod backup;
mod blog;
mod build;
mod cache;
mod deploy;
mod extension;
mod init_console;
mod link;
mod lobby;
mod open;
mod project;
mod query;
mod schema;
mod site;

pub use backup::BackupCommand;
pub use blog::BlogCommand;
pub use build::BuildCommand;
pub use cache::CacheArgs;
pub use deploy::DeployArgs;
pub use extension::ExtensionCommand;
pub use link::LinkCommand;
pub use lobby::LobbyCommand;
pub use open::OpenArgs;
pub use project::ProjectCommand;
pub use query::QueryCommand;
pub use schema::SchemaCommand;
pub use site::SiteCommand;

use crate::client::Client;
use crate::output::Output;
use crate::sites;
use crate::{Cli, Command};
use oxibuilder_core::extension::{CliCommand, CliHandler};
use std::collections::BTreeMap;
use std::sync::Arc;

// ───────────────────────── dispatch ─────────────────────────

pub async fn dispatch(cli: Cli) -> anyhow::Result<()> {
    let out = Output::new(cli.json);
    let sites_file = sites::load_sites();

    // Resolve active site name from --site flag (or OXIBUILDER_SITE, or default_site).
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

    // Open doesn't need HTTP — just open browser
    match cli.command {
        Command::Init { wizard } => {
            init_console::init(&out, cli.config.as_deref())?;
            if wizard {
                init_console::console(None, false, cli.config.as_deref()).await
            } else {
                Ok(())
            }
        }
        Command::Status => init_console::status(&out, &client).await,
        Command::Console { port, preview } => {
            init_console::console(port, preview, cli.config.as_deref()).await
        }
        Command::Blog(c) => blog::blog(c, &out).await,
        Command::Open { port } => open::open(OpenArgs { port }, &out),
        Command::Project(c) => project::project(c, &out).await,
        Command::Link(c) => link::link(c, &out).await,
        Command::Lobby(c) => lobby::lobby(c, &out, &client).await,
        Command::Extension(c) => extension::extension(c, &out, &client).await,
        Command::Backup(c) => backup::backup(c).await,
        Command::Build(c) => build::build(c).await,
        Command::Cache(c) => cache::cache(c, &out, &client).await,
        Command::Deploy(c) => deploy::deploy(c, &out).await,
        Command::Query(c) => query::query(c).await,
        Command::Schema(c) => schema::schema(c).await,
        Command::Site(_) => unreachable!(), // handled above
        Command::Dynamic(ref args) => dispatch_dynamic(args, &client, &out).await,
    }
}

// ──────────────────────── resolution ────────────────────────

/// Resolve active site name: `--site` flag > OXIBUILDER_SITE env > default_site > None.
/// Returns `Err` only when `--site` is given but the named site doesn't exist.
fn resolve_site_name<'a>(
    cli_site: Option<&'a str>,
    sites_file: &'a sites::SitesFile,
) -> anyhow::Result<Option<&'a str>> {
    // 1. --site flag (explicit, highest priority)
    if let Some(name) = cli_site
        && !name.is_empty()
    {
        if sites_file.exists(name) {
            return Ok(Some(name));
        }
        anyhow::bail!("site '{name}' not found — use `oxibuilder site add` to create it");
    }
    // 2. OXIBUILDER_SITE env — 명시적이므로 flag와 동일하게 존재 여부 검증
    if let Ok(env) = std::env::var("OXIBUILDER_SITE")
        && !env.is_empty()
    {
        if sites_file.exists(&env) {
            return Ok(sites_file.resolve_name(None));
        }
        anyhow::bail!(
            "site '{env}' (from OXIBUILDER_SITE env) not found — use `oxibuilder site add` to create it"
        );
    }
    // 3. default_site (from sites.toml)
    Ok(sites_file.resolve_name(None))
}

fn resolve_endpoint(
    cli_endpoint: Option<String>,
    _site_name: Option<&str>,
    _sites_file: &sites::SitesFile,
    config_path: Option<&std::path::Path>,
) -> anyhow::Result<String> {
    // 1. --endpoint flag (explicit override, highest priority)
    if let Some(e) = cli_endpoint
        && !e.is_empty()
    {
        return Ok(e);
    }
    // 2. OXIBUILDER_ENDPOINT env (legacy)
    if let Ok(e) = std::env::var("OXIBUILDER_ENDPOINT")
        && !e.is_empty()
    {
        return Ok(e);
    }
    // 3. oxibuilder.toml [site].base_url
    let cfg_path = config_path
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("oxibuilder.toml"));
    if cfg_path.exists()
        && let Ok(cfg) = oxibuilder_core::config::Config::load(&cfg_path)
        && !cfg.site.base_url.is_empty()
    {
        return Ok(cfg.site.base_url);
    }
    // 4. Hard-coded fallback
    Ok("http://127.0.0.1:8787".into())
}

fn resolve_token(
    cli_token: Option<String>,
    _site_name: Option<&str>,
    _sites_file: &sites::SitesFile,
) -> anyhow::Result<Option<String>> {
    // 1. --token flag (explicit override)
    if let Some(t) = cli_token
        && !t.is_empty()
    {
        return Ok(Some(t));
    }
    // 2. OXIBUILDER_TOKEN env (legacy)
    if let Ok(t) = std::env::var("OXIBUILDER_TOKEN")
        && !t.is_empty()
    {
        return Ok(Some(t));
    }
    Ok(None)
}

/// config/env에서 data_dir 해상. build.rs와 동일 로직.
pub(super) fn resolve_data_dir() -> anyhow::Result<std::path::PathBuf> {
    let config_path = std::env::var("OXIBUILDER_CONFIG")
        .map(std::path::PathBuf::from)
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
            return Ok(std::path::PathBuf::from(data_dir));
        }
    }
    if let Ok(dir) = std::env::var("OXIBUILDER_DATA_DIR") {
        return Ok(std::path::PathBuf::from(dir));
    }
    Ok(std::path::PathBuf::from("data"))
}

// ──────────────────── 동적 명령 디스패치 (doc/11) ────────────────────

/// Dynamic args를 받아 확장 레지스트리에서 명령을 조회하고 실행한다.
async fn dispatch_dynamic(args: &[String], client: &Client, out: &Output) -> anyhow::Result<()> {
    if args.is_empty() {
        anyhow::bail!(
            "missing dynamic command name.\nRun `oxibuilder --help` for available commands."
        );
    }
    let ext_name = &args[0];
    let sub_name = args.get(1).ok_or_else(|| {
        anyhow::anyhow!(
            "missing subcommand for extension '{ext_name}'.\nRun `oxibuilder {ext_name} --help` for available subcommands."
        )
    })?;
    let parsed = parse_dynamic_args(&args[2..]);

    let registry = resolve_command_registry(client).await?;
    let result = registry.lookup(ext_name, sub_name)?;

    match result {
        LookupResult::Native(handler) => handler.run(parsed, client).await,
        LookupResult::Proxy => {
            // 핸들러 없음 → 서버 위임 (WASM 확장)
            let resp = client
                .post(
                    &format!("/api/console/cli/exec/{ext_name}/{sub_name}"),
                    &serde_json::json!({ "args": parsed }),
                )
                .await
                .map_err(|e| anyhow::anyhow!("CLI exec proxy failed: {e}"))?;
            out.data(resp, "result")?;
            Ok(())
        }
    }
}

/// 동적 명령 레지스트리: 컴파일 확장 목록 + 서버 디스커버리 병합.
async fn resolve_command_registry(client: &Client) -> anyhow::Result<DynamicRegistry> {
    // 1. 컴파일 확장의 CLI 커맨드
    let compiled: Vec<CliCommand> = oxibuilder_console::all_extensions()
        .iter()
        .flat_map(|ext| ext.cli_commands())
        .collect();

    // 2. 서버 디스커버리 (실패 시 조용히 폴백)
    let discovered = match client.get("/api/console/cli/commands").await {
        Ok(val) => {
            let manifest: oxibuilder_core::extension::CliCommandManifest = serde_json::from_value(
                val,
            )
            .unwrap_or(oxibuilder_core::extension::CliCommandManifest { extensions: vec![] });
            manifest
        }
        Err(_) => oxibuilder_core::extension::CliCommandManifest { extensions: vec![] },
    };

    Ok(DynamicRegistry {
        compiled,
        discovered,
    })
}

/// `--key value --flag` 형태의 raw args를 `BTreeMap<String, String>`으로 파싱.
fn parse_dynamic_args(raw: &[String]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    let mut i = 0;
    while i < raw.len() {
        let key = &raw[i];
        if let Some(stripped) = key.strip_prefix("--") {
            if i + 1 < raw.len() && !raw[i + 1].starts_with("--") {
                map.insert(stripped.to_string(), raw[i + 1].clone());
                i += 2;
            } else {
                // 플래그 (값 없음): --flag = true
                map.insert(stripped.to_string(), "true".to_string());
                i += 1;
            }
        } else if let Some(stripped) = key.strip_prefix('-') {
            // short flag: -v (boolean) or -t value
            if i + 1 < raw.len() && !raw[i + 1].starts_with('-') {
                map.insert(stripped.to_string(), raw[i + 1].clone());
                i += 2;
            } else {
                map.insert(stripped.to_string(), "true".to_string());
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    map
}

/// 컴파일 확장 + 서버 디스커버리를 합친 레지스트리.
struct DynamicRegistry {
    compiled: Vec<CliCommand>,
    discovered: oxibuilder_core::extension::CliCommandManifest,
}

/// lookup 결과: 핸들러 소유권 여부.
enum LookupResult {
    /// 네이티브 핸들러 있음 (컴파일 확장).
    Native(Arc<dyn CliHandler>),
    /// 서버 위임 필요 (WASM 확장 또는 핸들러 없는 명령).
    Proxy,
}

impl DynamicRegistry {
    fn lookup(&self, ext_name: &str, sub_name: &str) -> anyhow::Result<LookupResult> {
        // 1. 컴파일 목록에서 검색
        if let Some(cmd) = self.compiled.iter().find(|c| c.name == ext_name)
            && let Some(sub) = cmd.subcommands.iter().find(|s| s.name == sub_name)
            && let Some(h) = &sub.handler
        {
            return Ok(LookupResult::Native(Arc::clone(h)));
        }
        // 2. 서버 디스커버리 목록에서 검색 (WASM 확장)
        if let Some(spec) = self
            .discovered
            .extensions
            .iter()
            .find(|e| e.name == ext_name || e.extension_id == ext_name)
            && spec.subcommands.iter().any(|s| s.name == sub_name)
        {
            return Ok(LookupResult::Proxy);
        }

        anyhow::bail!(
            "unknown command: 'oxibuilder {ext_name} {sub_name}'. \
             Run `oxibuilder --help` for available commands."
        )
    }
}

// ───────────────────────── unit tests ─────────────────────────
// Env-dependent paths (OXIBUILDER_ENDPOINT, OXIBUILDER_TOKEN) tested in E2E smoke.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sites::SitesFile;

    fn empty_sites() -> SitesFile {
        SitesFile::default()
    }

    fn sites_with_one(name: &str) -> SitesFile {
        let mut s = SitesFile::default();
        s.sites.insert(
            name.to_string(),
            sites::SiteEntry {
                path: std::path::PathBuf::from("/tmp/oxibuilder/").join(name),
            },
        );
        s.default_site = Some(name.to_string());
        s
    }

    // ─── resolve_endpoint ───

    #[test]
    fn endpoint_cli_flag_wins() {
        let sites = empty_sites();
        let result =
            resolve_endpoint(Some("https://cli.example.com".into()), None, &sites, None).unwrap();
        assert_eq!(result, "https://cli.example.com");
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
    fn token_none_when_nothing_configured() {
        let sites = empty_sites();
        let result = resolve_token(None, None, &sites).unwrap();
        assert_eq!(result, None);
    }

    // ─── resolve_site_name ───

    #[test]
    fn site_name_flag_valid() {
        let sites = sites_with_one("prod");
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
        let sites = sites_with_one("default-site");
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
