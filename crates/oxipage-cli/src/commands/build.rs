use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct BuildCommand {
    /// Output directory (default: data/out)
    #[arg(long)]
    pub out_dir: Option<String>,
}

pub(crate) async fn build(c: BuildCommand) -> anyhow::Result<()> {
    let BuildCommand {
        out_dir: custom_out,
    } = c;

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
    // 5. Write output (sources SPA bundle from the embedded binary, not CWD).
    //    BuildInputs carries site.base_url (drives deployment_base) + theme_id.
    let out_path = custom_out
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("out"));
    let config_path = std::env::var("OXIPAGE_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("oxipage.toml"));
    let config = if config_path.exists() {
        oxipage_core::config::Config::load(&config_path)?
    } else {
        oxipage_core::config::Config::default()
    };
    let theme_id = oxipage_core::theme::active_theme_id(&pool).await;
    let inputs =
        oxipage_core::builder::BuildInputs::new(&config.site.base_url, theme_id, "oxipage");
    oxipage_core::build_writer::write_build_output(&output, &out_path, &media_dir, &inputs)
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    // Emit the lobby manifest as static JSON so `fetchManifest()` resolves in static mode
    // (`pathToStaticFile('/lobby/manifest')` → `/data/lobby.json`). Uses the same assembly as
    // the live `/api/console/lobby/manifest` handler — one shape, no drift.
    // (config already loaded above for BuildInputs; reused here.)
    let extensions = oxipage_console::all_extensions();
    let manifest = oxipage_core::manifest::assemble(
        &pool,
        &config,
        &config.site.name,
        &config.site.base_url,
        &extensions,
    )
    .await;
    std::fs::write(
        out_path.join("data").join("lobby.json"),
        serde_json::to_string_pretty(&manifest)?,
    )?;

    println!("Build complete:");
    println!("  pages:     {}", output.pages.len());
    println!("  search:    {} docs", output.search_docs.len());
    println!("  output:    {}", out_path.display());
    println!();
    println!("  Preview:  oxipage console --preview");
    println!("  Deploy:   oxipage deploy --target github-pages");

    Ok(())
}
