use crate::client::Client;
use crate::output::Output;
use clap::Subcommand;
use serde_json::json;
use super::require_token;

#[derive(Subcommand, Debug)]
pub enum BackupCommand {
    /// SQLite VACUUM INTO 포인트-인-타임 스냅샷 (doc/05 §5.4).
    /// 서버 측 data_dir/backups/oxipage-<epoch>.db 에 일관된 복사본을 생성한다.
    /// admin 스코프 토큰 필요.
    Snapshot,
}


pub(crate) async fn backup(
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
