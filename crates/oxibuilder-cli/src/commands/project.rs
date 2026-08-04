use crate::output::Output;
use clap::Subcommand;
use oxibuilder_ext_projects::model::ProjectInput;
use oxibuilder_ext_projects::repo;

#[derive(Subcommand, Debug, Clone)]
pub enum ProjectCommand {
    /// 새 프로젝트 추가 (초안).
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
    /// 초안 발행.
    Publish { slug: String },
    /// 목록.
    List {
        #[arg(long, help = "상태 필터 (wip/active/archived)")]
        status: Option<String>,
        #[arg(long, help = "JSON 출력")]
        json: bool,
    },
    /// 단건 조회.
    Show { slug: String },
}

pub(crate) fn parse_link_pairs(
    pairs: &[String],
) -> anyhow::Result<serde_json::Map<String, serde_json::Value>> {
    let mut map = serde_json::Map::new();
    for p in pairs {
        let mut parts = p.splitn(2, '=');
        let key = parts.next().unwrap_or("").to_string();
        let val = parts.next().unwrap_or("").to_string();
        map.insert(key, serde_json::Value::String(val));
    }
    Ok(map)
}

pub(crate) async fn project(c: ProjectCommand, out: &Output) -> anyhow::Result<()> {
    let data_dir = super::resolve_data_dir()?;
    let pool = oxibuilder_core::db::connect(&data_dir.join("oxibuilder.db")).await?;

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
            let description_ko = match desc_ko {
                Some(p) => Some(std::fs::read_to_string(p)?),
                None => None,
            };
            let description_en = match desc_en {
                Some(p) => Some(std::fs::read_to_string(p)?),
                None => None,
            };
            let links = serde_json::Value::Object(parse_link_pairs(&link)?);
            let input = ProjectInput {
                title_ko,
                title_en,
                description_ko,
                description_en,
                tech_stack,
                status,
                started_at: None,
                ended_at: None,
                links,
                featured,
                slug: None,
            };
            let slug_base = repo::slugify(input.title_en.as_deref(), input.title_ko.as_deref());
            let resolved_slug = repo::ensure_unique_slug(&pool, &slug_base).await?;
            let project = repo::create(&pool, &input, &resolved_slug).await?;
            if publish {
                let published = repo::publish(&pool, &project.slug).await?;
                out.data(serde_json::to_value(&published)?, "published")
            } else {
                out.data(serde_json::to_value(&project)?, "draft created")
            }
        }
        ProjectCommand::Publish { slug } => {
            let project = repo::publish(&pool, &slug).await?;
            out.data(serde_json::to_value(&project)?, "published")
        }
        ProjectCommand::List { status, json: _ } => {
            let projects = repo::list(&pool, status.as_deref(), 200, false).await?;
            out.data(serde_json::to_value(&projects)?, "projects")
        }
        ProjectCommand::Show { slug } => {
            let project = repo::find_by_slug(&pool, &slug)
                .await?
                .ok_or_else(|| anyhow::anyhow!("project not found: {slug}"))?;
            out.data(serde_json::to_value(&project)?, "project")
        }
    }
}
