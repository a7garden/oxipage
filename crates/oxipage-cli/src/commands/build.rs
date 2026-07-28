use clap::Subcommand;
use std::path::PathBuf;
use anyhow::Context;

#[derive(Subcommand, Debug, Clone)]
pub enum BuildCommand {
    /// Build the static site from the local SQLite database.
    /// Generates out/ directory with HTML + JSON + assets.
    #[command(name = "build")]
    Run {
        /// Output directory (default: data/out)
        #[arg(long)]
        out_dir: Option<String>,
    },
}

pub(crate) async fn build(c: BuildCommand) -> anyhow::Result<()> {
    match c {
        BuildCommand::Run { out_dir: custom_out } => {
            // 1. Resolve data directory from config
            let data_dir = resolve_data_dir()?;
            let db_path = data_dir.join("oxipage.db");
            let media_dir = data_dir.join("media");
            let web_dist = PathBuf::from("web/dist");

            if !db_path.exists() {
                anyhow::bail!("Database not found at {}. Is the server initialized?", db_path.display());
            }

            // 2. Connect to the database
            println!("Building site...");
            println!("  db:      {}", db_path.display());
            println!("  media:   {}", media_dir.display());

            let pool = oxipage_core::db::connect(&db_path).await?;

            // 3. Get all builders
            let builders = oxipage_console::all_builders();
            let builder_refs: Vec<Box<dyn oxipage_core::builder::BuildExt>> = builders;

            // 4. Run build pipeline
            let output = oxipage_core::build::build_site(&pool, &builder_refs)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            // 5. Write output
            let out_path = custom_out
                .map(PathBuf::from)
                .unwrap_or_else(|| data_dir.join("out"));
            oxipage_core::build_writer::write_build_output(
                &output,
                &out_path,
                &media_dir,
                &web_dist,
            )
            .map_err(|e| anyhow::anyhow!("{}", e))?;

            println!("Build complete:");
            println!("  pages:     {}", output.pages.len());
            println!("  search:    {} docs", output.search_docs.len());
            println!("  output:    {}", out_path.display());
            println!();
            println!("  Preview:  oxipage console --preview");
            println!("  Deploy:   oxipage deploy --target github-pages");

            Ok(())
        }
    }
}

/// Resolve the data directory from config file or environment.
fn resolve_data_dir() -> anyhow::Result<PathBuf> {
    // Try OXIPAGE_CONFIG env → config file → default "data"
    let config_path = std::env::var("OXIPAGE_CONFIG")
        .map(PathBuf::from)
        .ok()
        .filter(|p| p.exists());

    if let Some(ref path) = config_path {
        let toml_str = std::fs::read_to_string(path)?;
        let value: toml::Value = toml::from_str(&toml_str)?;
        if let Some(data_dir) = value
            .get("server")
            .and_then(|s| s.get("data_dir"))
            .and_then(|d| d.as_str())
        {
            return Ok(PathBuf::from(data_dir));
        }
    }

    // Try OXIPAGE_DATA_DIR env
    if let Ok(dir) = std::env::var("OXIPAGE_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }

    // Default
    Ok(PathBuf::from("data"))
}
