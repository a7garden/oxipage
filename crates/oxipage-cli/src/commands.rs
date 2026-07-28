//! 서브커맨드 정의 + 디스패치.

use crate::client::Client;
use crate::credentials;
use crate::output::Output;
use crate::sites;
use crate::{Cli, Command};
use clap::Subcommand;
use serde_json::json;


#[derive(Subcommand, Debug)]
pub enum SiteCommand {
    /// 새 사이트 등록
    Add {
        name: String,
        #[arg(long)]
        endpoint: String,
        #[arg(long)]
        token: Option<String>,
        #[arg(long)]
        default: bool,
    },
    /// 사이트 목록
    List,
    /// 사이트 상세 정보
    Show {
        name: Option<String>,
    },
    /// 기본 사이트로 전환
    Use {
        name: String,
    },
    /// 사이트 정보 수정
    Edit {
        name: String,
        #[arg(long)]
        endpoint: Option<String>,
        #[arg(long)]
        token: Option<String>,
    },
    /// 사이트 삭제
    Rm {
        name: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum AuthCommand {
    /// (Phase 4 PAT 완비 전까지) 브라우저 로그인 대신 안내만 출력.
    Login,
    /// credentials 파일에 토큰 저장 (0600). OXIPAGE_TOKEN env 또는 이 파일에서 읽음.
    Set {
        /// 저장할 평문 토큰 (OXIPAGE_ADMIN_TOKEN 또는 PAT 평문).
        token: String,
    },
    /// credentials 파일에서 토큰 삭제.
    Unset,
    /// credentials 파일의 토큰 존재 여부.
    Status,
    /// 서버 측 PAT 관리 (doc/04 §4.3 `auth token create|list|revoke`).
    #[command(subcommand)]
    Token(TokenCommand),
}

#[derive(Subcommand, Debug)]
pub enum TokenCommand {
    /// 새 PAT 발급. admin 스코프 토큰(OXIPAGE_ADMIN_TOKEN 또는 admin PAT) 필요.
    /// 평문은 이때 한 번만 반환 — 즉시 안전한 곳에 보관.
    Create {
        #[arg(long)]
        label: String,
        #[arg(long, value_delimiter = ',', help = "쉼표 구분: post:write,post:publish,read")]
        scopes: Vec<String>,
    },
    /// 발급된 PAT 목록 (평문 제외, prefix만).
    List,
    /// PAT revoke.
    Revoke { id: i64 },
}

#[derive(Subcommand, Debug)]
pub enum BlogCommand {
    /// 새 초안 생성 (doc/04 §4.3 초안 우선 원칙: add/new는 초안만).
    New {
        title: String,
        #[arg(long, default_value = "ko")]
        lang: String,
        #[arg(long, help = "본문 마크다운 파일. 미지정 시 빈 본문")]
        file: Option<std::path::PathBuf>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        #[arg(long, help = "즉시 발행 (초안 우선 원칙 위반 — 명시적 승인)")]
        publish: bool,
    },
    /// 초안 발행 (별도 승인 단계).
    Publish { slug: String },
    /// 목록 (기본: 발행본만. --draft로 초안만).
    List {
        #[arg(long)]
        draft: bool,
        #[arg(long)]
        lang: Option<String>,
    },
    /// 단건 조회.
    Show { slug: String },
    /// 수정 (title/body/tags).
    Edit {
        slug: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long, help = "본문 마크다운 파일")]
        file: Option<std::path::PathBuf>,
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    /// 삭제.
    Rm { slug: String },
}

#[derive(Subcommand, Debug)]
pub enum ProjectCommand {
    Add {
        #[arg(long)]
        title_ko: Option<String>,
        #[arg(long)]
        title_en: Option<String>,
        #[arg(long, help = "한국어 설명 마크다운 파일")]
        desc_ko: Option<std::path::PathBuf>,
        #[arg(long, help = "영어 설명 마크다운 파일")]
        desc_en: Option<std::path::PathBuf>,
        #[arg(long = "tech")]
        tech_stack: Vec<String>,
        #[arg(long, help = "key=URL 형태 (예: repo=https://...). 반복 가능")]
        link: Vec<String>,
        #[arg(long, default_value = "wip")]
        status: String,
        #[arg(long)]
        featured: bool,
        #[arg(long, help = "즉시 발행")]
        publish: bool,
    },
    Publish { slug: String },
    List {
        #[arg(long)]
        status: Option<String>,
    },
    Show { slug: String },
}

#[derive(Subcommand, Debug)]
pub enum LinkCommand {
    Add {
        #[arg(long)]
        title: String,
        #[arg(long)]
        url: String,
        #[arg(long)]
        desc_ko: Option<String>,
        #[arg(long)]
        desc_en: Option<String>,
        #[arg(long = "tag")]
        tags: Vec<String>,
        #[arg(long)]
        featured: bool,
    },
    List,
    Rm { id: i64 },
}

#[derive(Subcommand, Debug)]
pub enum LobbyCommand {
    /// 확장별 로비 표시 모드 설정 (doc/03 §3.6).
    Layout {
        extension: String,
        #[arg(long)]
        mode: String,
    },
    /// 현재 로비 설정 조회.
    Config,
}

#[derive(Subcommand, Debug)]
pub enum ExtensionCommand {
    /// 설치된 확장 목록 + 활성/purge 상태
    List,
    /// 확장 활성화 (purge 상태였으면 복구 — 다음 부팅 시 마이그레이션 재실행)
    Enable { name: String },
    /// 확장 비활성화 (soft — 라우트 404 + FTS 색인 정리, DB/미디어 유지)
    Disable { name: String },
    /// 확장 완전 삭제 (테이블 DROP + 미디어 디렉토리 rm)
    Purge {
        name: String,
        #[arg(long)]
        yes: bool,
    },
    /// WASM 확장 런타임 설치 (doc/08 §8.4). data/extensions/<name>.wasm 저장.
    /// 활성화에는 --features wasm 으로 빌드된 서버 재기동 필요.
    Install { name: String },
}

#[derive(Subcommand, Debug)]
pub enum BackupCommand {
    /// SQLite VACUUM INTO 포인트-인-타임 스냅샷 (doc/05 §5.4).
    /// 서버 측 data_dir/backups/oxipage-<epoch>.db 에 일관된 복사본을 생성한다.
    /// admin 스코프 토큰 필요.
    Snapshot,
}

// ───────────────────────── dispatch ─────────────────────────

pub async fn dispatch(cli: Cli) -> anyhow::Result<()> {
    let out = Output::new(cli.json);
    let sites_file = sites::SitesFile::load();

    // Resolve active site name from --site flag (or OXIPAGE_SITE, or default_site).
    let site_name = resolve_site_name(cli.site.as_deref(), &sites_file)?;

    // Handle site management commands early — they don't need HTTP client.
    if let Command::Site(c) = &cli.command {
        return dispatch_site(c, &out, &sites_file, site_name).await;
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
        Command::Init => init(&out, cli.config.as_deref()),
        Command::Status => status(&out, &client).await,
        Command::Serve { port } => serve(port, cli.config.as_deref()).await,
        Command::Auth(c) => auth(c, &out, &client).await,
        Command::Blog(c) => blog(c, &out, &client).await,
        Command::Project(c) => project(c, &out, &client).await,
        Command::Link(c) => link(c, &out, &client).await,
        Command::Lobby(c) => lobby(c, &out, &client).await,
        Command::Extension(c) => extension(c, &out, &client).await,
        Command::Backup(c) => backup(c, &out, &client).await,
        Command::Site(_) => unreachable!(), // handled above
    }
}

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

fn require_token(client: &Client) -> anyhow::Result<()> {
    if !client.has_token() {
        anyhow::bail!(
            "no token found — set OXIPAGE_TOKEN env, pass --token, or run `oxipage auth token set <token>`"
        );
    }
    Ok(())
}

// ───────────────────────── site subcommands ─────────────────────────

async fn dispatch_site(
    cmd: &SiteCommand,
    out: &Output,
    sites_file: &sites::SitesFile,
    active_site: Option<&str>,
) -> anyhow::Result<()> {
    match cmd {
        SiteCommand::List => site_list(out, sites_file, active_site),
        SiteCommand::Show { name } => site_show(out, sites_file, name.as_deref(), active_site),
        SiteCommand::Use { name } => site_use(out, sites_file, name).await,
        SiteCommand::Add { name, endpoint, token, default } => {
            site_add(out, sites_file, name, endpoint, token.as_deref(), *default).await
        }
        SiteCommand::Edit { name, endpoint, token } => {
            site_edit(out, sites_file, name, endpoint.as_deref(), token.as_deref()).await
        }
        SiteCommand::Rm { name } => site_rm(out, sites_file, name).await,
    }
}

fn site_list(out: &Output, sites_file: &sites::SitesFile, active_site: Option<&str>) -> anyhow::Result<()> {
    if out.json {
        let list: Vec<serde_json::Value> = sites_file
            .site_names()
            .into_iter()
            .map(|name| {
                let entry = sites_file.get(&name);
                json!({
                    "name": name,
                    "endpoint": entry.map(|e| e.endpoint.as_str()).unwrap_or(""),
                    "active": Some(name.as_str()) == active_site,
                    "has_token": entry.and_then(|e| e.token.as_ref()).is_some(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&list)?);
    } else {
        let names = sites_file.site_names();
        if names.is_empty() {
            println!("no sites configured — use `oxipage site add`");
            return Ok(());
        }
        for name in &names {
            let entry = sites_file.get(name);
            let marker = if Some(name.as_str()) == active_site { "* " } else { "  " };
            let ep = entry.map(|e| e.endpoint.as_str()).unwrap_or("?");
            println!("{marker}{name}   {ep}");
        }
    }
    Ok(())
}

fn site_show(
    out: &Output,
    sites_file: &sites::SitesFile,
    name: Option<&str>,
    active_site: Option<&str>,
) -> anyhow::Result<()> {
    let target = name.or(active_site)
        .and_then(|n| sites_file.get(n).map(|e| (n, e)));
    let (name, entry) = match target {
        Some(pair) => pair,
        None => {
            anyhow::bail!("no active site and no site name given");
        }
    };
    if out.json {
        let masked = entry.token.as_ref().map(|t| {
            if t.len() > 8 {
                format!("{}...{}", &t[..6], &t[t.len()-3..])
            } else {
                "***".into()
            }
        });
        println!("{}", serde_json::to_string_pretty(&json!({
            "name": name,
            "endpoint": entry.endpoint,
            "token": masked,
            "active": Some(name) == active_site,
        }))?);
    } else {
        let active_label = if Some(name) == active_site { " (active)" } else { "" };
        println!("name:       {name}{active_label}");
        println!("endpoint:   {}", entry.endpoint);
        if let Some(tok) = &entry.token {
            let masked = if tok.len() > 8 {
                format!("{}...{}", &tok[..6], &tok[tok.len()-3..])
            } else {
                "***".into()
            };
            println!("token:      {masked}");
        } else {
            println!("token:      (none)");
        }
    }
    Ok(())
}

async fn site_use(
    out: &Output,
    sites_file: &sites::SitesFile,
    name: &str,
) -> anyhow::Result<()> {
    if !sites_file.exists(name) {
        anyhow::bail!("site '{name}' not found");
    }
    let mut new_sites = sites_file.clone();
    new_sites.default_site = Some(name.to_string());
    new_sites.save()?;
    if !out.json {
        println!("active site set to '{name}'");
    }
    Ok(())
}

async fn site_add(
    out: &Output,
    sites_file: &sites::SitesFile,
    name: &str,
    endpoint: &str,
    token: Option<&str>,
    default: bool,
) -> anyhow::Result<()> {
    if sites_file.exists(name) {
        anyhow::bail!("site '{name}' already exists — use `oxipage site edit` to update");
    }
    let mut new_sites = sites_file.clone();
    new_sites.sites.insert(
        name.to_string(),
        sites::SiteEntry {
            endpoint: endpoint.to_string(),
            token: token.map(String::from),
        },
    );
    if default || new_sites.default_site.is_none() {
        new_sites.default_site = Some(name.to_string());
    }
    new_sites.save()?;
    if !out.json {
        if default {
            println!("site '{name}' added and set as default");
        } else {
            println!("site '{name}' added");
        }
    }
    Ok(())
}

async fn site_edit(
    out: &Output,
    sites_file: &sites::SitesFile,
    name: &str,
    endpoint: Option<&str>,
    token: Option<&str>,
) -> anyhow::Result<()> {
    let _entry = sites_file.sites.get(name).ok_or_else(|| {
        anyhow::anyhow!("site '{name}' not found — use `oxipage site add` first")
    })?;
    let mut new_sites = sites_file.clone();
    let new_entry = new_sites.sites.get_mut(name).unwrap();
    if let Some(ep) = endpoint {
        new_entry.endpoint = ep.to_string();
    }
    // token=Some("") → clear token; token=Some("tok") → set; token=None → leave as-is
    match token {
        Some("") => new_entry.token = None,
        Some(t) => new_entry.token = Some(t.to_string()),
        None => {},
    }
    new_sites.save()?;
    if !out.json {
        println!("site '{name}' updated");
    }
    Ok(())
}

async fn site_rm(
    out: &Output,
    sites_file: &sites::SitesFile,
    name: &str,
) -> anyhow::Result<()> {
    if !sites_file.exists(name) {
        anyhow::bail!("site '{name}' not found");
    }
    let mut new_sites = sites_file.clone();
    new_sites.sites.remove(name);
    // If removed site was the default, pick the next available or clear.
    if new_sites.default_site.as_deref() == Some(name) {
        let next = new_sites.site_names().into_iter().next();
        new_sites.default_site = next;
    }
    new_sites.save()?;
    if !out.json {
        println!("site '{name}' removed");
    }
    Ok(())
}

// ───────────────────────── init / status / serve ─────────────────────────

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
enabled = ["profile"]

[lobby]
default_mode = "grid"
"#;

fn init(out: &Output, config_path: Option<&std::path::Path>) -> anyhow::Result<()> {
    let path = config_path
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("oxipage.toml"));
    if path.exists() {
        anyhow::bail!("{} already exists", path.display());
    }
    std::fs::write(&path, DEFAULT_TOML)?;
    out.ok(format!("wrote {}", path.display()))
}

async fn status(out: &Output, client: &Client) -> anyhow::Result<()> {
    let health = client
        .get("/healthz")
        .await
        .map_err(|e| anyhow::anyhow!("healthz failed: {e}"))?;
    let manifest = client
        .get("/api/v1/lobby/manifest")
        .await
        .map_err(|e| anyhow::anyhow!("manifest failed: {e}"))?;

    if out.json {
        let summary = json!({
            "endpoint": client.endpoint(),
            "health": health,
            "manifest": manifest,
            "authenticated": client.has_token(),
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
    println!(
        "authenticated: {}",
        if client.has_token() { "yes" } else { "no" }
    );
    Ok(())
}

async fn serve(port: Option<u16>, _config_path: Option<&std::path::Path>) -> anyhow::Result<()> {
    if let Some(p) = port {
        // SAFETY: CLI는 단일 스레드 진입점이며 set_var 직후 run_server가 값을 읽어
        // SocketAddr에 반영한다. 다른 스레드가 환경변수를 경쟁 읽지 않는다.
        #[allow(unsafe_code)]
        unsafe {
            std::env::set_var("OXIPAGE_PORT", p.to_string());
        }
    }
    oxipage_server::run_server().await
}

// ───────────────────────── auth ─────────────────────────

async fn auth(
    c: AuthCommand,
    out: &Output,
    client: &Client,
) -> anyhow::Result<()> {
    match c {
        AuthCommand::Login => out.ok(
            "auth login (브라우저)는 향후 지원. 지금은 서버에 OXIPAGE_ADMIN_TOKEN 설정 후 `oxipage auth set <token>` 또는 `oxipage auth token create --label ... --scopes post:write`.",
        ),
        AuthCommand::Set { token } => {
            credentials::store_token(&token)?;
            out.ok("token stored to credentials file (0600)")
        }
        AuthCommand::Unset => {
            credentials::clear_token()?;
            out.ok("stored token cleared")
        }
        AuthCommand::Status => {
            if credentials::load_token()?.is_some() {
                out.ok("a token is stored")
            } else {
                out.ok("no token stored")
            }
        }
        AuthCommand::Token(tc) => {
            require_token(client)?;
            match tc {
                TokenCommand::Create { label, scopes } => {
                    let payload = json!({ "label": label, "scopes": scopes });
                    let res = client.post_raw("/api/v1/auth/tokens", payload).await?;
                    if out.json {
                        println!("{}", serde_json::to_string_pretty(&res)?);
                    } else {
                        let plain = res
                            .get("data")
                            .and_then(|d| d.get("plain_token"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("?");
                        println!("PAT created. 평문 토큰 (다시 안 보임 — 즉시 보관):");
                        println!("  {plain}");
                        println!("  OXIPAGE_TOKEN={plain}  (또는 `oxipage auth set {plain}`)");
                    }
                    Ok(())
                }
                TokenCommand::List => {
                    let res = client.get("/api/v1/auth/tokens").await?;
                    out.data(res, "tokens")
                }
                TokenCommand::Revoke { id } => {
                    let res = client.delete(&format!("/api/v1/auth/tokens/{id}")).await?;
                    out.data(res, "revoked")
                }
            }
        }
    }
}

// ───────────────────────── blog ─────────────────────────

async fn blog(
    c: BlogCommand,
    out: &Output,
    client: &Client,
) -> anyhow::Result<()> {
    require_token(client)?;
    match c {
        BlogCommand::New {
            title,
            lang,
            file,
            tags,
            publish,
        } => {
            let body = match file {
                Some(p) => std::fs::read_to_string(&p)?,
                None => String::new(),
            };
            let payload = json!({
                "title": title,
                "body": body,
                "lang": lang,
                "tags": tags,
            });
            let res = client.post_raw("/api/v1/blog", payload).await?;
            let data = Client::unwrap_data(res)?;
            let slug = data.get("slug").and_then(|s| s.as_str()).unwrap_or("");
            if publish && !slug.is_empty() {
                let pub_res = client
                    .post_raw(&format!("/api/v1/blog/{slug}/publish"), json!({}))
                    .await?;
                out.data(pub_res, "published")
            } else {
                out.data(json!({ "data": data }), "draft created")
            }
        }
        BlogCommand::Publish { slug } => {
            let res = client
                .post_raw(&format!("/api/v1/blog/{slug}/publish"), json!({}))
                .await?;
            out.data(res, "published")
        }
        BlogCommand::List { draft, lang } => {
            let mut path = "/api/v1/blog?".to_string();
            if draft {
                path.push_str("draft=true&");
            }
            if let Some(l) = lang {
                path.push_str(&format!("lang={l}&"));
            }
            let res = client.get(path.trim_end_matches('&')).await?;
            out.data(res, "posts")
        }
        BlogCommand::Show { slug } => {
            let res = client.get(&format!("/api/v1/blog/{slug}")).await?;
            out.data(res, "post")
        }
        BlogCommand::Edit {
            slug,
            title,
            file,
            tags,
        } => {
            let mut payload = serde_json::Map::new();
            if let Some(t) = title {
                payload.insert("title".into(), json!(t));
            }
            if let Some(p) = file {
                let body = std::fs::read_to_string(&p)?;
                payload.insert("body".into(), json!(body));
            }
            if !tags.is_empty() {
                payload.insert("tags".into(), json!(tags));
            }
            let res = client
                .patch(
                    &format!("/api/v1/blog/{slug}"),
                    &serde_json::Value::Object(payload),
                )
                .await?;
            out.data(res, "updated")
        }
        BlogCommand::Rm { slug } => {
            let res = client.delete(&format!("/api/v1/blog/{slug}")).await?;
            out.data(res, "deleted")
        }
    }
}

// ───────────────────────── project ─────────────────────────

fn parse_link_pairs(
    pairs: &[String],
) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
    let mut map = serde_json::Map::new();
    for p in pairs {
        let (k, v) = p
            .split_once('=')
            .ok_or_else(|| anyhow::anyhow!("link must be key=URL form: {p}"))?;
        map.insert(k.to_string(), json!(v));
    }
    Ok(map)
}

async fn project(
    c: ProjectCommand,
    out: &Output,
    client: &Client,
) -> anyhow::Result<()> {
    require_token(client)?;
    match c {
        ProjectCommand::Add {
            title_ko,
            title_en,
            desc_ko,
            desc_en,
            tech_stack,
            link,
            status,
            featured,
            publish,
        } => {
            let description_ko = desc_ko.map(|p| std::fs::read_to_string(&p)).transpose()?;
            let description_en = desc_en.map(|p| std::fs::read_to_string(&p)).transpose()?;
            let links = if link.is_empty() {
                serde_json::Map::new()
            } else {
                parse_link_pairs(&link)?
            };
            let payload = json!({
                "title_ko": title_ko,
                "title_en": title_en,
                "description_ko": description_ko,
                "description_en": description_en,
                "tech_stack": tech_stack,
                "status": status,
                "links": serde_json::Value::Object(links),
                "featured": featured,
            });
            let res = client.post_raw("/api/v1/projects", payload).await?;
            let data = Client::unwrap_data(res)?;
            let slug = data.get("slug").and_then(|s| s.as_str()).unwrap_or("");
            if publish && !slug.is_empty() {
                let pub_res = client
                    .post_raw(&format!("/api/v1/projects/{slug}/publish"), json!({}))
                    .await?;
                out.data(pub_res, "published")
            } else {
                out.data(json!({ "data": data }), "project created")
            }
        }
        ProjectCommand::Publish { slug } => {
            let res = client
                .post_raw(&format!("/api/v1/projects/{slug}/publish"), json!({}))
                .await?;
            out.data(res, "published")
        }
        ProjectCommand::List { status } => {
            let path = match status {
                Some(s) => format!("/api/v1/projects?status={s}"),
                None => "/api/v1/projects".to_string(),
            };
            let res = client.get(&path).await?;
            out.data(res, "projects")
        }
        ProjectCommand::Show { slug } => {
            let res = client.get(&format!("/api/v1/projects/{slug}")).await?;
            out.data(res, "project")
        }
    }
}

// ───────────────────────── link ─────────────────────────

async fn link(
    c: LinkCommand,
    out: &Output,
    client: &Client,
) -> anyhow::Result<()> {
    require_token(client)?;
    match c {
        LinkCommand::Add {
            title,
            url,
            desc_ko,
            desc_en,
            tags,
            featured,
        } => {
            let payload = json!({
                "title": title,
                "url": url,
                "description_ko": desc_ko,
                "description_en": desc_en,
                "tags": tags,
                "featured": featured,
            });
            let res = client.post_raw("/api/v1/links", payload).await?;
            out.data(res, "link added")
        }
        LinkCommand::List => {
            let res = client.get("/api/v1/links").await?;
            out.data(res, "links")
        }
        LinkCommand::Rm { id } => {
            let res = client.delete(&format!("/api/v1/links/{id}")).await?;
            out.data(res, "deleted")
        }
    }
}

// ───────────────────────── lobby ─────────────────────────

async fn lobby(
    c: LobbyCommand,
    out: &Output,
    client: &Client,
) -> anyhow::Result<()> {
    match c {
        LobbyCommand::Layout { extension, mode } => {
            if !matches!(mode.as_str(), "canvas" | "grid" | "list") {
                anyhow::bail!("mode must be canvas|grid|list (got {mode})");
            }
            require_token(&client)?;
            let payload = json!({ "display_mode": mode });
            let res = client
                .put(&format!("/api/v1/lobby/config/{extension}"), &payload)
                .await?;
            out.data(res, "lobby config updated")
        }
        LobbyCommand::Config => {
            let res = client.get("/api/v1/lobby/config").await?;
            out.data(res, "lobby config")
        }
    }
}

// ───────────────────────── extension ─────────────────────────

async fn extension(
    c: ExtensionCommand,
    out: &Output,
    client: &Client,
) -> anyhow::Result<()> {
    match c {
        ExtensionCommand::List => {
            require_token(&client)?;
            let res = client.get("/api/v1/extensions").await?;
            out.data(res, "extensions")
        }
        ExtensionCommand::Enable { name } => {
            require_token(&client)?;
            let res = client
                .post_raw(&format!("/api/v1/extensions/{name}/enable"), json!({}))
                .await?;
            out.data(res, "extension enabled")
        }
        ExtensionCommand::Disable { name } => {
            require_token(&client)?;
            let res = client
                .post_raw(&format!("/api/v1/extensions/{name}/disable"), json!({}))
                .await?;
            out.data(res, "extension disabled")
        }
        ExtensionCommand::Purge { name, yes } => {
            require_token(&client)?;
            if !yes {
                anyhow::bail!(
                    "purge is destructive — pass --yes to confirm (drops tables + removes media for '{name}')"
                );
            }
            let res = client.delete(&format!("/api/v1/extensions/{name}")).await?;
            out.data(res, "extension purged")
        }
        ExtensionCommand::Install { name } => {
            require_token(&client)?;
            let res = client
                .post_raw("/api/v1/extensions/install", json!({ "name": name }))
                .await?;
            out.data(res, "extension install")
        }
    }
}

async fn backup(
    c: BackupCommand,
    out: &Output,
    client: &Client,
) -> anyhow::Result<()> {
    match c {
        BackupCommand::Snapshot => {
            require_token(&client)?;
            let res = client
                .post_raw("/api/v1/backup/snapshot", json!({}))
                .await?;
            out.data(res, "backup snapshot")
        }
    }
}

// ───────────────────────── unit tests ─────────────────────────
// Env-dependent paths (OXIPAGE_ENDPOINT, OXIPAGE_TOKEN) tested in E2E smoke,
// not here — avoids parallel-test race on process env.

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
