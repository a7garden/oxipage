//! Oxipage CLI — 모든 명령은 인증된 HTTP 호출 (doc/04 §4.1).
//!
//! 유일한 예외: `oxipage serve`가 서버 프로세스를 직접 기동한다.

mod client;
mod commands;
mod credentials;
mod output;
mod sites;

use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "oxipage",
    version,
    about = "Oxipage personal-site CLI — every command is an authenticated HTTP call",
    long_about = "doc/04 §4.1: CLI는 API의 레퍼런스 클라이언트. 모든 쓰기/읽기는 인증된 HTTP 호출이다.\n유일한 예외는 serve 자체가 서버 프로세스를 기동하는 것뿐이다."
)]
pub struct Cli {
    #[arg(long, global = true, env = "OXIPAGE_ENDPOINT")]
    pub endpoint: Option<String>,
    #[arg(long, global = true, env = "OXIPAGE_SITE")]
    pub site: Option<String>,
    #[arg(long, global = true, env = "OXIPAGE_TOKEN")]
    pub token: Option<String>,

    #[arg(long, global = true)]
    pub json: bool,

    #[arg(long, global = true, env = "OXIPAGE_CONFIG")]
    pub config: Option<PathBuf>,

    #[arg(long, global = true, env = "OXIPAGE_TLS_INSECURE")]
    pub insecure: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// oxipage.toml 스캐폴딩
    Init,
    /// 초안/최근 게시물/서버 상태 요약
    Status,
    /// 로컬 개발 서버 기동 (유일하게 HTTP를 거치지 않는 예외)
    Serve {
        #[arg(long)]
        port: Option<u16>,
    },
    /// 인증 (PAT 체계는 Phase 4; Phase 1은 OXIPAGE_TOKEN/credentials 저장만)
    #[command(subcommand)]
    Auth(commands::AuthCommand),
    /// 블로그 (doc/02 §2.6)
    #[command(subcommand)]
    Blog(commands::BlogCommand),
    /// 프로젝트 포트폴리오 (doc/02 §2.4)
    #[command(subcommand)]
    Project(commands::ProjectCommand),
    /// 생태계 링크 (doc/02 §2.11)
    #[command(subcommand)]
    Link(commands::LinkCommand),
    /// 로비 표시 설정 (doc/03 §3.6)
    #[command(subcommand)]
    Lobby(commands::LobbyCommand),
    /// 확장 탑재/제거 (doc/02 §2.13, doc/04 §4.3)
    #[command(subcommand)]
    Extension(commands::ExtensionCommand),
    /// 백업 (doc/05 §5.4) — SQLite VACUUM INTO 스냅샷
    #[command(subcommand)]
    Backup(commands::BackupCommand),
    /// 사이트 프로필 관리 (doc/09) — 접속 대상 서버 등록/전환
    #[command(subcommand)]
    Site(commands::SiteCommand),
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    commands::dispatch(cli).await
}
