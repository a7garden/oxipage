use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct BuildCommand {
    /// Output directory (default: data/out)
    #[arg(long)]
    pub out_dir: Option<String>,
}

pub(crate) async fn build(c: BuildCommand) -> anyhow::Result<()> {
    let BuildCommand { out_dir: custom_out } = c;

    // 1. Resolve data directory from config
    let data_dir = super::resolve_data_dir()?;
    let db_path = data_dir.join("oxipage.db");
    let media_dir = data_dir.join("media");

    if !db_path.exists() {
        anyhow::bail!(
            "Database not found at {}. Is the server initialized?",
            db_path.display()
        );
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

    // 5. Write output (sources SPA bundle from the embedded binary, not CWD)
    let out_path = custom_out
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("out"));
    oxipage_core::build_writer::write_build_output(&output, &out_path, &media_dir)
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
