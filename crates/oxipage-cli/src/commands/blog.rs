use clap::Subcommand;
use oxipage_ext_blog::model::{BlogPatch, BlogPostInput};
use oxipage_ext_blog::repo;
use crate::output::Output;

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

pub(crate) async fn blog(c: BlogCommand, out: &Output) -> anyhow::Result<()> {
    let data_dir = super::resolve_data_dir()?;
    let pool = oxipage_core::db::connect(&data_dir.join("oxipage.db")).await?;

    match c {
        BlogCommand::New { title, lang, file, tags, publish } => {
            let body = match file {
                Some(p) => std::fs::read_to_string(p)?,
                None => String::new(),
            };
            let input = BlogPostInput {
                title,
                body,
                lang,
                tags,
                translation_group_id: None,
                slug: None,
            };
            let slug_base = repo::slugify(&input.title);
            let resolved_slug = repo::ensure_unique_slug(&pool, &slug_base).await?;
            let post = repo::create(&pool, &input, &resolved_slug).await?;
            if publish {
                let published = repo::publish(&pool, &post.slug).await?;
                out.data(serde_json::to_value(&published)?, "published")
            } else {
                out.data(serde_json::to_value(&post)?, "draft created")
            }
        }
        BlogCommand::Publish { slug } => {
            let post = repo::publish(&pool, &slug).await?;
            out.data(serde_json::to_value(&post)?, "published")
        }
        BlogCommand::List { draft, lang } => {
            let posts = repo::list(&pool, draft, lang.as_deref(), 200).await?;
            out.data(serde_json::to_value(&posts)?, "posts")
        }
        BlogCommand::Show { slug } => {
            let post = repo::find_by_slug(&pool, &slug)
                .await?
                .ok_or_else(|| anyhow::anyhow!("post not found: {slug}"))?;
            out.data(serde_json::to_value(&post)?, "post")
        }
        BlogCommand::Edit { slug, title, file, tags } => {
            let body = match file {
                Some(p) => Some(std::fs::read_to_string(p)?),
                None => None,
            };
            let tags_opt = if tags.is_empty() { None } else { Some(tags) };
            let patch = BlogPatch { title, body, lang: None, tags: tags_opt };
            let post = repo::update(&pool, &slug, &patch)
                .await?
                .ok_or_else(|| anyhow::anyhow!("post not found: {slug}"))?;
            out.data(serde_json::to_value(&post)?, "updated")
        }
        BlogCommand::Rm { slug } => {
            let removed = repo::delete(&pool, &slug).await?;
            if removed {
                out.ok(format!("deleted: {slug}"))
            } else {
                anyhow::bail!("post not found: {slug}")
            }
        }
    }
}
