use crate::client::Client;
use crate::credentials;
use crate::output::Output;
use clap::Subcommand;
use serde_json::json;
use super::require_token;

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

pub(crate) async fn auth(
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

