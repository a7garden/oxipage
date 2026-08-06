use crate::output::Output;
use clap::Subcommand;
use oxibuilder_ext_profile::{model::ProfileInput, repo};

#[derive(Subcommand, Debug, Clone)]
pub enum ProfileCommand {
    /// Update profile fields. Only provided flags change; others are kept.
    Set {
        #[arg(long)]
        display_name: Option<String>,
        #[arg(long)]
        email: Option<String>,
        #[arg(long = "github")]
        github_username: Option<String>,
        #[arg(long)]
        linkedin: Option<String>,
        #[arg(long)]
        tagline_ko: Option<String>,
        #[arg(long)]
        tagline_en: Option<String>,
        #[arg(long)]
        avatar_url: Option<String>,
        #[arg(long)]
        bio_ko: Option<String>,
        #[arg(long)]
        bio_en: Option<String>,
    },
    /// Print the current profile as JSON.
    Show,
}

pub(crate) async fn profile(c: ProfileCommand, out: &Output) -> anyhow::Result<()> {
    let data_dir = super::resolve_data_dir()?;
    let pool = oxibuilder_core::db::connect(&data_dir.join("oxibuilder.db")).await?;

    match c {
        ProfileCommand::Show => match repo::get(&pool).await? {
            Some(p) => out.data(serde_json::to_value(&p)?, "profile"),
            None => out.ok("no profile set"),
        },
        ProfileCommand::Set {
            display_name,
            email,
            github_username,
            linkedin,
            tagline_ko,
            tagline_en,
            avatar_url,
            bio_ko,
            bio_en,
        } => {
            // Ensure the singleton row exists (server boot creates it from
            // site.name; this is the CLI's safety net for a fresh DB).
            repo::ensure_singleton(&pool, "Owner").await?;
            let cur = repo::get(&pool)
                .await?
                .ok_or_else(|| anyhow::anyhow!("profile row missing after ensure_singleton"))?;

            // Merge: CLI flags override current values; unset flags keep them.
            let input = ProfileInput {
                expected_updated_at: String::new(),
                display_name: display_name.unwrap_or(cur.display_name),
                tagline_ko: tagline_ko.or(cur.tagline_ko),
                tagline_en: tagline_en.or(cur.tagline_en),
                avatar_url: avatar_url.or(cur.avatar_url),
                bio_ko: bio_ko.or(cur.bio_ko),
                bio_en: bio_en.or(cur.bio_en),
                email: email.or(cur.email),
                github_username: github_username.or(cur.github_username),
                linkedin_url: linkedin.or(cur.linkedin_url),
                education: cur.education,
                custom_links: cur.custom_links,
            };
            let updated = repo::upsert(&pool, &input).await?;
            out.data(serde_json::to_value(&updated)?, "profile updated")
        }
    }
}
