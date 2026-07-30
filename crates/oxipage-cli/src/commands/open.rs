//! `oxipage open` — 실행 중인 서버의 URL을 브라우저로 오픈.

use crate::output::Output;
use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct OpenArgs {
    /// 커스텀 포트
    #[arg(long)]
    pub port: Option<u16>,
}

pub(crate) fn open(args: OpenArgs, out: &Output) -> anyhow::Result<()> {
    let port = args.port.unwrap_or(8787);
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
