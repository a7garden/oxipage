use crate::client::Client;
use crate::output::Output;
use clap::Subcommand;
use serde_json::json;
use super::require_token;

#[derive(Subcommand, Debug, Clone)]
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


pub(crate) async fn link(
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
