//! 토큰 저장 — OXIPAGE_TOKEN env → ~/.config/oxipage/credentials (0600) (doc/04 §4.2).
//!
//! Phase 1: 임시 단일 토큰(OXIPAGE_ADMIN_TOKEN과 동일 값)을 파일에 저장.
//! Phase 4: PAT 체계(token_id, scope, hash)로 확장.

use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

fn credentials_path() -> Result<PathBuf> {
    let proj = directories::ProjectDirs::from("dev", "oxipage", "oxipage")
        .context("could not determine config directory")?;
    Ok(proj.config_dir().join("credentials"))
}

/// env → 파일 순서로 토큰 조회.
pub fn load_token() -> Result<Option<String>> {
    if let Ok(t) = std::env::var("OXIPAGE_TOKEN")
        && !t.is_empty()
    {
        return Ok(Some(t));
    }
    let p = credentials_path()?;
    if !p.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
    let trimmed = raw.trim().to_string();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(trimmed))
    }
}

/// credentials 파일에 토큰 저장 (0600). PAT 완비 전까지 사용자가 직접 값으로 저장.
pub fn store_token(token: &str) -> Result<()> {
    let p = credentials_path()?;
    if let Some(parent) = p.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("creating {}", parent.display()))?;
    }
    fs::write(&p, token).with_context(|| format!("writing {}", p.display()))?;
    set_mode_0600(&p)?;
    Ok(())
}

pub fn clear_token() -> Result<()> {
    let p = credentials_path()?;
    if p.exists() {
        fs::remove_file(&p).with_context(|| format!("removing {}", p.display()))?;
    }
    Ok(())
}

#[cfg(unix)]
fn set_mode_0600(p: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(p)?.permissions();
    perms.set_mode(0o600);
    fs::set_permissions(p, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_mode_0600(_p: &std::path::Path) -> Result<()> {
    Ok(())
}
