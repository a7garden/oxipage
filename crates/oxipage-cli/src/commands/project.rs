use crate::client::Client;
use crate::output::Output;
use clap::Subcommand;
use serde_json::json;
use super::require_token;

#[derive(Subcommand, Debug, Clone)]
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


pub(crate) fn parse_link_pairs(
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

pub(crate) async fn project(
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
