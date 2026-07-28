use crate::client::Client;
use crate::output::Output;
use clap::Subcommand;
use serde_json::json;
use super::require_token;

#[derive(Subcommand, Debug, Clone)]
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


pub(crate) async fn blog(
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
