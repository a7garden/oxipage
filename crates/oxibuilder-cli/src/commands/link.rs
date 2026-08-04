use crate::output::Output;
use clap::Subcommand;
use oxibuilder_ext_links::model::LinkCardInput;
use oxibuilder_ext_links::repo;

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
    Rm {
        id: i64,
    },
}

pub(crate) async fn link(c: LinkCommand, out: &Output) -> anyhow::Result<()> {
    let data_dir = super::resolve_data_dir()?;
    let pool = oxibuilder_core::db::connect(&data_dir.join("oxibuilder.db")).await?;

    match c {
        LinkCommand::Add {
            title,
            url,
            desc_ko,
            desc_en,
            tags,
            featured,
        } => {
            let input = LinkCardInput {
                title,
                url,
                description_ko: desc_ko,
                description_en: desc_en,
                thumbnail_url: None,
                tags,
                display_order: 0,
                featured,
            };
            let card = repo::create(&pool, &input).await?;
            out.data(serde_json::to_value(&card)?, "link added")
        }
        LinkCommand::List => {
            let cards = repo::list(&pool, None, 500).await?;
            out.data(serde_json::to_value(&cards)?, "links")
        }
        LinkCommand::Rm { id } => {
            let removed = repo::delete(&pool, id).await?;
            if removed {
                out.ok(format!("deleted link {id}"))
            } else {
                anyhow::bail!("link not found: {id}")
            }
        }
    }
}
