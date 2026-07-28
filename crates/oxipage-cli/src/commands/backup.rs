use clap::Subcommand;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Subcommand, Debug)]
pub enum BackupCommand {
    /// SQLite VACUUM INTO 포인트-인-타임 스냅샷.
    /// 로컬 DB의 일관된 복사본을 data_dir/backups/ 에 생성한다.
    Snapshot,
}

pub(crate) async fn backup(c: BackupCommand) -> anyhow::Result<()> {
    match c {
        BackupCommand::Snapshot => {
            // 1. Resolve data directory
            let data_dir = resolve_data_dir()?;
            let db_path = data_dir.join("oxipage.db");

            if !db_path.exists() {
                anyhow::bail!("Database not found at {}", db_path.display());
            }

            // 2. Create backup directory
            let backup_dir = data_dir.join("backups");
            tokio::fs::create_dir_all(&backup_dir).await?;

            // 3. Generate snapshot filename
            let epoch = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let snap_name = format!("oxipage-{epoch}.db");
            let snap_path = backup_dir.join(&snap_name);

            println!(
                "Backup: {} → {}",
                db_path.display(),
                snap_path.display()
            );

            // 4. Connect to DB and run VACUUM INTO
            let pool = oxipage_core::db::connect(&db_path).await?;
            oxipage_core::backup::vacuum_into(&pool, &snap_path).await?;

            println!("Backup complete: {} ({} bytes)", snap_name, snap_path.metadata().map(|m| m.len()).unwrap_or(0));
            Ok(())
        }
    }
}

fn resolve_data_dir() -> anyhow::Result<PathBuf> {
    // Same logic as build.rs
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

    if let Ok(dir) = std::env::var("OXIPAGE_DATA_DIR") {
        return Ok(PathBuf::from(dir));
    }

    Ok(PathBuf::from("data"))
}
