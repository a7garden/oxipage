use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct SchemaCommand {
    /// Filter by extension name
    #[arg(long)]
    pub extension: Option<String>,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(serde::Serialize)]
struct ColumnInfo {
    name: String,
    r#type: String,
    pk: bool,
    nullable: bool,
}

#[derive(serde::Serialize)]
struct TableInfo {
    name: String,
    columns: Vec<ColumnInfo>,
}

pub(crate) async fn schema(
    c: SchemaCommand,
) -> anyhow::Result<()> {
    // Resolve DB path
    let config_path = std::env::var("OXIPAGE_CONFIG")
        .map(PathBuf::from)
        .ok()
        .filter(|p| p.exists());

    let data_dir = if let Some(ref path) = config_path {
        let toml_str = std::fs::read_to_string(path)?;
        let config: serde_json::Value = toml::from_str(&toml_str)?;
        config["server"]["data_dir"]
            .as_str()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("data"))
    } else {
        PathBuf::from("data")
    };

    let db_path = data_dir.join("oxipage.db");
    let conn = rusqlite::Connection::open(&db_path)?;

    // Query all tables
    let table_pattern = c.extension.as_deref()
        .map(|ext| format!("%{}%", ext))
        .unwrap_or_else(|| "%".to_string());

    let mut stmt = conn.prepare(
        "SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name LIKE ?1 ORDER BY name"
    )?;
    let table_names: Vec<String> = stmt
        .query_map([&table_pattern], |row| row.get(0))?
        .filter_map(|r| r.ok())
        .collect();

    let mut tables = Vec::new();

    for name in &table_names {
        let mut col_stmt = conn.prepare(&format!("PRAGMA table_info({})", name))?;
        let columns: Vec<ColumnInfo> = col_stmt
            .query_map([], |row| {
                Ok(ColumnInfo {
                    name: row.get(1)?,
                    r#type: row.get(2)?,
                    pk: row.get::<_, i32>(5)? != 0,
                    nullable: row.get::<_, i32>(3)? == 0,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        tables.push(TableInfo {
            name: name.clone(),
            columns,
        });
    }

    if c.json {
        println!("{}", serde_json::to_string_pretty(&serde_json::json!({ "tables": tables }))?);
    } else {
        for table in &tables {
            println!("{}", table.name);
            for col in &table.columns {
                let pk = if col.pk { " PK" } else { "" };
                let nullable = if col.nullable { "" } else { " NOT NULL" };
                println!("  {col_name:<30} {col_type:<15}{pk}{nullable}",
                    col_name = col.name,
                    col_type = col.r#type);
            }
            println!();
        }
    }

    Ok(())
}
