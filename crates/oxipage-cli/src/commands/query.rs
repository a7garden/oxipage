use clap::Args;
use std::path::PathBuf;

#[derive(Args, Debug, Clone)]
pub struct QueryCommand {
    /// SQL query string (SELECT only)
    pub sql: String,
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub(crate) async fn query(c: QueryCommand) -> anyhow::Result<()> {
    if !is_read_only(&c.sql) {
        anyhow::bail!("Only SELECT queries are allowed for safety");
    }

    // Resolve DB path: OXIPAGE_CONFIG → oxipage.toml → default data dir
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
    if !db_path.exists() {
        anyhow::bail!("Database not found at {}", db_path.display());
    }

    // Open SQLite connection (synchronous)
    let conn = rusqlite::Connection::open(&db_path)?;

    let mut stmt = conn.prepare(&c.sql)?;
    let col_count = stmt.column_count();
    let col_names: Vec<String> = (0..col_count)
        .map(|i| stmt.column_name(i).unwrap_or("?").to_string())
        .collect();

    let rows = stmt.query_map([], |row| {
        let mut map = serde_json::Map::new();
        for (i, name) in col_names.iter().enumerate() {
            let val: rusqlite::types::Value = row.get_unwrap(i);
            let json_val = rusqlite_val_to_json(val);
            map.insert(name.clone(), json_val);
        }
        Ok(map)
    })?;

    let mut results: Vec<serde_json::Value> = Vec::new();
    for row in rows {
        results.push(serde_json::Value::Object(row?));
    }

    if c.json {
        println!("{}", serde_json::to_string_pretty(&results)?);
    } else {
        // Table format
        if results.is_empty() {
            println!("(0 rows)");
            return Ok(());
        }
        // Print header
        println!("{}", col_names.join(" | "));
        println!(
            "{}",
            col_names
                .iter()
                .map(|_| "---")
                .collect::<Vec<_>>()
                .join(" | ")
        );
        for row in &results {
            let vals: Vec<String> = col_names
                .iter()
                .map(|name| {
                    row.get(name)
                        .and_then(|v| v.as_str().map(|s| s.to_string()))
                        .unwrap_or_else(|| "NULL".into())
                })
                .collect();
            println!("{}", vals.join(" | "));
        }
        println!("({} rows)", results.len());
    }

    Ok(())
}

fn is_read_only(sql: &str) -> bool {
    let trimmed = sql.trim().to_uppercase();
    trimmed.starts_with("SELECT") || trimmed.starts_with("PRAGMA") || trimmed.starts_with("EXPLAIN")
}

fn rusqlite_val_to_json(val: rusqlite::types::Value) -> serde_json::Value {
    match val {
        rusqlite::types::Value::Null => serde_json::Value::Null,
        rusqlite::types::Value::Integer(i) => serde_json::json!(i),
        rusqlite::types::Value::Real(f) => serde_json::json!(f),
        rusqlite::types::Value::Text(s) => serde_json::json!(s),
        rusqlite::types::Value::Blob(b) => serde_json::json!(base64_encode(&b)),
    }
}

fn base64_encode(data: &[u8]) -> String {
    // Simple base64 without adding a dependency
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        result.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
        result.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
        if chunk.len() > 1 {
            result.push(CHARS[((triple >> 6) & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
        if chunk.len() > 2 {
            result.push(CHARS[(triple & 0x3F) as usize] as char);
        } else {
            result.push('=');
        }
    }
    result
}
