//! `oxipage open` — 실행 중인 서버의 URL을 브라우저로 오픈 (doc/13 §13.4.2).

use clap::Args;
use crate::output::Output;

#[derive(Args, Debug)]
pub struct OpenArgs {
    /// Admin 콘솔 오픈 (기본: 메인 사이트 :8787)
    #[arg(long)]
    pub admin: bool,
    /// 커스텀 포트
    #[arg(long)]
    pub port: Option<u16>,
}

pub(crate) fn open(args: OpenArgs, out: &Output) -> anyhow::Result<()> {
    let port = if args.admin {
        args.port.unwrap_or(8788)
    } else {
        args.port.unwrap_or(8787)
    };
    let url = format!("http://127.0.0.1:{port}");

    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(target_os = "windows") {
        "start"
    } else {
        "xdg-open"
    };

    match std::process::Command::new(cmd).arg(&url).spawn() {
        Ok(_) => out.ok(format!("opened {url}")),
        Err(e) => {
            eprintln!("could not open browser: {e}");
            eprintln!("open {url} manually");
            Ok(())
        }
    }
}
