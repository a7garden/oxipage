use crate::client::Client;
use crate::output::Output;
use clap::Subcommand;
use serde_json::json;


#[derive(Subcommand, Debug, Clone)]
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


pub(crate) async fn lobby(
    c: LobbyCommand,
    out: &Output,
    client: &Client,
) -> anyhow::Result<()> {
    match c {
        LobbyCommand::Layout { extension, mode } => {
            if !matches!(mode.as_str(), "canvas" | "grid" | "list") {
                anyhow::bail!("mode must be canvas|grid|list (got {mode})");
            }
            
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
