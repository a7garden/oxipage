use crate::client::Client;
use crate::output::Output;
use clap::Subcommand;
use serde_json::json;


#[derive(Subcommand, Debug, Clone)]
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


pub(crate) async fn extension(
    c: ExtensionCommand,
    out: &Output,
    client: &Client,
) -> anyhow::Result<()> {
    match c {
        ExtensionCommand::List => {
            
            let res = client.get("/api/v1/extensions").await?;
            out.data(res, "extensions")
        }
        ExtensionCommand::Enable { name } => {
            
            let res = client
                .post_raw(&format!("/api/v1/extensions/{name}/enable"), json!({}))
                .await?;
            out.data(res, "extension enabled")
        }
        ExtensionCommand::Disable { name } => {
            
            let res = client
                .post_raw(&format!("/api/v1/extensions/{name}/disable"), json!({}))
                .await?;
            out.data(res, "extension disabled")
        }
        ExtensionCommand::Purge { name, yes } => {
            
            if !yes {
                anyhow::bail!(
                    "purge is destructive — pass --yes to confirm (drops tables + removes media for '{name}')"
                );
            }
            let res = client.delete(&format!("/api/v1/extensions/{name}")).await?;
            out.data(res, "extension purged")
        }
        ExtensionCommand::Install { name } => {
            
            let res = client
                .post_raw("/api/v1/extensions/install", json!({ "name": name }))
                .await?;
            out.data(res, "extension install")
        }
    }
}
