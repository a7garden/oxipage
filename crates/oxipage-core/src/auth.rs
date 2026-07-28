//! 인증 (doc/01 §1.8, doc/04 §4.2).
//!
//! 두 경로:
//! - **OXIPAGE_ADMIN_TOKEN** (부트스트랩 슈퍼유저): 서버 환경변수 단일 토큰.
//!   통과 시 `scopes = ["admin"]` — 모든 `require_scope` 통과.
//! - **PAT** (Personal Access Token): DB `auth_token` 테이블. 평문 ≥256비트,
//!   SHA-256 해시 저장. `post:write`/`post:publish`/`read` 스코프.

use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// 쓰기 라우트의 기본 extractor. 검증된 토큰의 scopes를 들고 다닌다.
/// 핸들러는 `auth.require_scope("post:publish")?`로 세분 검사.
pub struct AdminAuth {
    pub token_id: Option<i64>,
    pub scopes: Vec<String>,
}

impl AdminAuth {
    /// 해당 스코프(또는 `admin` 슈퍼유저 스코프)가 있으면 OK, 아니면 403.
    pub fn require_scope(&self, scope: &str) -> Result<(), ApiError> {
        if self.scopes.iter().any(|s| s == scope || s == "admin") {
            Ok(())
        } else {
            Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "insufficient_scope",
                &format!("this action requires the '{scope}' scope"),
            ))
        }
    }
}

impl FromRequestParts<AppState> for AdminAuth {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        // (1) bearer 추출. None이면 서버에 토큰 자체가 없을 때만 503, 그 외 401.
        // (1) 서버에 토큰 설정이 전혀 없으면 모든 쓰기 503 (bearer 유무 무관).
        if state.admin_token.is_none() && pat_count(state).await == 0 {
            return Err(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "admin_not_configured",
                "server has no OXIPAGE_ADMIN_TOKEN and no PAT issued; run `oxipage auth token create`",
            ));
        }
        // (2) bearer 추출. 없으면 401.
        let token = match bearer_token(parts) {
            Some(t) => t,
            None => return Err(unauthorized()),
        };

        // (2) OXIPAGE_ADMIN_TOKEN 슈퍼유저.
        if let Some(expected) = &state.admin_token
            && constant_time_eq(token.as_bytes(), expected.as_bytes())
        {
            return Ok(AdminAuth {
                token_id: None,
                scopes: vec!["admin".into()],
            });
        }

        // (3) PAT 검증. AdminAuth 진입 자체가 post:write 필요 (read 전용 PAT 거부).
        match verify_pat(state, &token).await.map_err(ApiError::internal)? {
            Some((id, scopes)) => {
                if !scopes.iter().any(|s| s == "post:write" || s == "admin") {
                    return Err(ApiError::new(
                        StatusCode::FORBIDDEN,
                        "insufficient_scope",
                        "this route requires the 'post:write' scope",
                    ));
                }
                Ok(AdminAuth {
                    token_id: Some(id),
                    scopes,
                })
            }
            None => Err(unauthorized()),
        }
    }
}

fn bearer_token(parts: &Parts) -> Option<String> {
    parts
        .headers
        .get(AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

async fn pat_count(state: &AppState) -> i64 {
    let row: Result<(i64,), sqlx::Error> =
        sqlx::query_as("SELECT COUNT(*) FROM auth_token WHERE revoked_at IS NULL")
            .fetch_one(&state.db)
            .await;
    row.map(|r| r.0).unwrap_or(0)
}

/// PAT 평문 검증. 성공 시 (token_id, scopes) + last_used_at 갱신.
pub async fn verify_pat(
    state: &AppState,
    plain: &str,
) -> anyhow::Result<Option<(i64, Vec<String>)>> {
    let hash = sha256_hex(plain.as_bytes());
    let row: Option<(i64, String)> = sqlx::query_as(
        "SELECT id, scopes FROM auth_token
         WHERE token_hash = ? AND revoked_at IS NULL",
    )
    .bind(&hash)
    .fetch_optional(&state.db)
    .await?;

    let Some((id, scopes_json)) = row else {
        return Ok(None);
    };
    let scopes: Vec<String> = serde_json::from_str(&scopes_json).unwrap_or_default();

    // last_used_at 갱신 (best-effort).
    let _ = sqlx::query(
        "UPDATE auth_token SET last_used_at = strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id = ?",
    )
    .bind(id)
    .execute(&state.db)
    .await;

    Ok(Some((id, scopes)))
}

/// 새 PAT 발급. `oxp_<base64url(32바이트)>` 평문 반환 + DB 저장.
pub fn generate_plain_token() -> String {
    let mut bytes = [0u8; 32];
    fill_random(&mut bytes);
    format!("oxp_{}", base64url_encode(&bytes))
}

fn fill_random(buf: &mut [u8]) {
    #[cfg(unix)]
    {
        use std::io::Read;
        if let Ok(mut f) = std::fs::File::open("/dev/urandom")
            && f.read_exact(buf).is_ok()
        {
            return;
        }
    }
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    for (i, b) in buf.iter_mut().enumerate() {
        *b = ((ts.wrapping_mul((i as u128) + 1)) & 0xff) as u8;
    }
}

fn base64url_encode(bytes: &[u8]) -> String {
    const ALPHA: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8) | (bytes[i + 2] as u32);
        out.push(ALPHA[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 12) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 6) & 0x3f) as usize] as char);
        out.push(ALPHA[(n & 0x3f) as usize] as char);
        i += 3;
    }
    let rem = bytes.len() - i;
    if rem == 1 {
        let n = (bytes[i] as u32) << 16;
        out.push(ALPHA[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 12) & 0x3f) as usize] as char);
    } else if rem == 2 {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8);
        out.push(ALPHA[((n >> 18) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 12) & 0x3f) as usize] as char);
        out.push(ALPHA[((n >> 6) & 0x3f) as usize] as char);
    }
    out
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for b in digest {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct PatRow {
    pub id: i64,
    pub label: String,
    pub token_prefix: String,
    pub scopes: String,
    pub created_at: String,
    pub last_used_at: Option<String>,
    pub revoked_at: Option<String>,
}

/// PAT 생성. (평문, row) 반환. 평문은 이때 한 번만.
pub async fn create_pat(
    state: &AppState,
    label: &str,
    scopes: &[String],
) -> anyhow::Result<(String, PatRow)> {
    let plain = generate_plain_token();
    let hash = sha256_hex(plain.as_bytes());
    let prefix: String = plain.chars().take(12).collect();
    let scopes_json = serde_json::to_string(scopes)?;
    let row: PatRow = sqlx::query_as(
        "INSERT INTO auth_token (label, token_hash, token_prefix, scopes)
         VALUES (?, ?, ?, ?)
         RETURNING id, label, token_prefix, scopes, created_at, last_used_at, revoked_at",
    )
    .bind(label)
    .bind(&hash)
    .bind(&prefix)
    .bind(&scopes_json)
    .fetch_one(&state.db)
    .await?;
    Ok((plain, row))
}

pub async fn list_pats(state: &AppState) -> anyhow::Result<Vec<PatRow>> {
    let rows: Vec<PatRow> = sqlx::query_as(
        "SELECT id, label, token_prefix, scopes, created_at, last_used_at, revoked_at
         FROM auth_token ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

pub async fn revoke_pat(state: &AppState, id: i64) -> anyhow::Result<bool> {
    let res = sqlx::query(
        "UPDATE auth_token SET revoked_at = strftime('%Y-%m-%dT%H:%M:%fZ','now')
         WHERE id = ? AND revoked_at IS NULL",
    )
    .bind(id)
    .execute(&state.db)
    .await?;
    Ok(res.rows_affected() > 0)
}

pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn unauthorized() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "unauthorized",
        "missing or invalid bearer token",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_hex_is_stable() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn generated_token_has_prefix_and_length() {
        let t = generate_plain_token();
        assert!(t.starts_with("oxp_"));
        assert!(t.len() >= 40);
    }

    #[test]
    fn constant_time_eq_compares_bytes() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(!constant_time_eq(b"", b"a"));
    }

    #[test]
    fn require_scope_admin_passes_all() {
        let a = AdminAuth {
            token_id: None,
            scopes: vec!["admin".into()],
        };
        assert!(a.require_scope("post:write").is_ok());
        assert!(a.require_scope("post:publish").is_ok());
    }

    #[test]
    fn require_scope_exact_match() {
        let a = AdminAuth {
            token_id: Some(1),
            scopes: vec!["post:write".into()],
        };
        assert!(a.require_scope("post:write").is_ok());
        assert!(a.require_scope("post:publish").is_err());
    }
}
