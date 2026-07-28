//! sites.toml CRUD API 핸들러.
//!
//! `~/.config/oxipage/sites.toml`을 직접 읽고 쓴다. 토큰은 목록에서 마스킹.
//! 파일 권한: Unix에서 0600.

use crate::{AdminContext, AdminError};
use axum::extract::{Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

// ──────────────────────────── data model ────────────────────────────

/// `sites.toml` 최상위 구조.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SitesFile {
    #[serde(default)]
    pub default_site: Option<String>,
    #[serde(default)]
    pub sites: BTreeMap<String, SiteEntry>,
}

/// 단일 사이트 엔트리.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SiteEntry {
    pub endpoint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

impl SitesFile {
    pub(crate) fn load(path: &PathBuf) -> Result<Self, AdminError> {
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = fs::read_to_string(path).map_err(|e| {
            AdminError::Internal(anyhow::anyhow!("failed to read sites.toml: {e}"))
        })?;
        toml::from_str(&raw).map_err(|e| {
            AdminError::BadRequest(format!("invalid sites.toml: {e}"))
        })
    }

    fn save(&self, path: &PathBuf) -> Result<(), AdminError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                AdminError::Internal(anyhow::anyhow!("failed to create config dir: {e}"))
            })?;
        }
        let raw = toml::to_string_pretty(self).map_err(|e| {
            AdminError::Internal(anyhow::anyhow!("failed to serialize: {e}"))
        })?;
        // Write to temp file first, then rename for atomicity
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, &raw).map_err(|e| {
            AdminError::Internal(anyhow::anyhow!("failed to write sites.toml: {e}"))
        })?;
        fs::rename(&tmp, path).map_err(|e| {
            AdminError::Internal(anyhow::anyhow!("failed to rename sites.toml: {e}"))
        })?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(meta) = fs::metadata(path) {
                let mut perms = meta.permissions();
                perms.set_mode(0o600);
                let _ = fs::set_permissions(path, perms);
            }
        }
        Ok(())
    }
}

impl Default for SitesFile {
    fn default() -> Self {
        Self {
            default_site: None,
            sites: BTreeMap::new(),
        }
    }
}

// ──────────────────────────── response types ────────────────────────────

/// 목록 응답에서 토큰을 마스킹한 사이트 정보.
#[derive(Serialize)]
pub struct SiteListItem {
    pub name: String,
    pub endpoint: String,
    pub token_masked: Option<String>,
    pub active: bool,
}



// ──────────────────────────── request types ────────────────────────────

#[derive(Deserialize)]
pub struct AddSiteRequest {
    pub name: String,
    pub endpoint: String,
    pub token: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateSiteRequest {
    pub endpoint: Option<String>,
    pub token: Option<String>,
}

#[derive(Deserialize)]
pub struct SetActiveRequest {
    pub name: String,
}

// ──────────────────────────── handlers ────────────────────────────

/// 토큰 마스킹: 앞 8자 + "..." or "none"
fn mask_token(token: &Option<String>) -> Option<String> {
    token.as_ref().map(|t| {
        if t.len() > 8 {
            format!("{}...", &t[..8])
        } else {
            t.clone()
        }
    })
}

/// GET /api/admin/sites — 사이트 목록
pub(crate) async fn list(
    State(ctx): State<AdminContext>,
) -> Result<Json<serde_json::Value>, AdminError> {
    let sf = SitesFile::load(&ctx.sites_path)?;
    let items: Vec<SiteListItem> = sf
        .sites
        .iter()
        .map(|(name, entry)| SiteListItem {
            name: name.clone(),
            endpoint: entry.endpoint.clone(),
            token_masked: mask_token(&entry.token),
            active: Some(name.as_str())
                == sf.default_site.as_deref(),
        })
        .collect();
    Ok(Json(serde_json::json!({"data": items})))
}

/// POST /api/admin/sites — 사이트 추가
pub(crate) async fn add(
    State(ctx): State<AdminContext>,
    Json(input): Json<AddSiteRequest>,
) -> Result<Json<serde_json::Value>, AdminError> {
    let name = input.name.trim().to_string();
    if name.is_empty() {
        return Err(AdminError::BadRequest("name is required".into()));
    }
    let endpoint = input.endpoint.trim().to_string();
    if endpoint.is_empty() {
        return Err(AdminError::BadRequest("endpoint is required".into()));
    }

    let mut sf = SitesFile::load(&ctx.sites_path)?;
    if sf.sites.contains_key(&name) {
        return Err(AdminError::BadRequest(format!(
            "site '{name}' already exists"
        )));
    }

    sf.sites.insert(
        name.clone(),
        SiteEntry {
            endpoint,
            token: input.token.map(|t| t.trim().to_string()).filter(|t| !t.is_empty()),
        },
    );

    // 첫 사이트면 자동으로 기본값
    if sf.default_site.is_none() {
        sf.default_site = Some(name.clone());
    }

    sf.save(&ctx.sites_path)?;
    Ok(Json(serde_json::json!({"status": "ok", "name": name})))
}

/// PUT /api/admin/sites/{name} — 사이트 수정
pub(crate) async fn update(
    State(ctx): State<AdminContext>,
    Path(name): Path<String>,
    Json(input): Json<UpdateSiteRequest>,
) -> Result<Json<serde_json::Value>, AdminError> {
    let mut sf = SitesFile::load(&ctx.sites_path)?;
    let entry = sf
        .sites
        .get_mut(&name)
        .ok_or_else(|| AdminError::NotFound(format!("site '{name}' not found")))?;

    if let Some(ep) = input.endpoint {
        let ep = ep.trim().to_string();
        if ep.is_empty() {
            return Err(AdminError::BadRequest("endpoint cannot be empty".into()));
        }
        entry.endpoint = ep;
    }
    if input.token.is_some() {
        let t = input.token.unwrap_or_default();
        entry.token = if t.is_empty() {
            None
        } else {
            Some(t)
        };
    }

    sf.save(&ctx.sites_path)?;
    Ok(Json(serde_json::json!({"status": "ok", "name": name})))
}

/// DELETE /api/admin/sites/{name} — 사이트 삭제
pub(crate) async fn delete(
    State(ctx): State<AdminContext>,
    Path(name): Path<String>,
) -> Result<Json<serde_json::Value>, AdminError> {
    let mut sf = SitesFile::load(&ctx.sites_path)?;
    if sf.sites.remove(&name).is_none() {
        return Err(AdminError::NotFound(format!("site '{name}' not found")));
    }

    // 기본 사이트가 삭제됐으면 첫 번째 사이트로 변경
    if sf.default_site.as_deref() == Some(&name) {
        sf.default_site = sf.sites.keys().next().cloned();
    }

    sf.save(&ctx.sites_path)?;
    Ok(Json(serde_json::json!({"status": "ok", "name": name})))
}

/// GET /api/admin/sites/active — 현재 활성 사이트
pub(crate) async fn get_active(
    State(ctx): State<AdminContext>,
) -> Result<Json<serde_json::Value>, AdminError> {
    let sf = SitesFile::load(&ctx.sites_path)?;
    Ok(Json(serde_json::json!({
        "data": {
            "name": sf.default_site
        }
    })))
}

/// PUT /api/admin/sites/active — 활성 사이트 전환
pub(crate) async fn set_active(
    State(ctx): State<AdminContext>,
    Json(input): Json<SetActiveRequest>,
) -> Result<Json<serde_json::Value>, AdminError> {
    let mut sf = SitesFile::load(&ctx.sites_path)?;
    if !sf.sites.contains_key(&input.name) {
        return Err(AdminError::NotFound(format!("site '{}' not found", input.name)));
    }
    sf.default_site = Some(input.name.clone());
    sf.save(&ctx.sites_path)?;
    Ok(Json(serde_json::json!({"status": "ok", "name": input.name})))
}
