use crate::client::GithubClient;
use crate::model::{GithubEvent, ListQuery};
use crate::repo;
use axum::Json;
use axum::body::Bytes;
use axum::extract::{Extension, Query};
use axum::http::{HeaderMap, StatusCode};
use hmac::{Hmac, Mac};

use oxipage_core::error::ApiError;
use oxipage_core::extension::DataEnvelope;
use oxipage_core::state::SiteScopedDb;
use sha2::Sha256;

type HmacSha256 = Hmac<Sha256>;

pub async fn list(
    Extension(pool): Extension<SiteScopedDb>,
    Query(query): Query<ListQuery>,
) -> Result<Json<DataEnvelope<Vec<crate::model::ActivityEvent>>>, ApiError> {
    let events = repo::list(&pool.db, query.repo.as_deref(), query.limit.unwrap_or(30))
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(DataEnvelope { data: events }))
}

/// GitHub webhook 수신 (doc/01 §1.9, doc/02 §2.8). 공개 라우트이므로
/// **HMAC-SHA256 서명 검증이 필수** — `X-Hub-Signature-256` 헤더를
/// `OXIPAGE_GITHUB_WEBHOOK_SECRET`으로 검증한다.
///
/// **구현 (doc/08 수정):** 이전엔 서명 검증 없이 JSON을 그대로 받아
/// 누구나 가짜 이벤트를 주입할 수 있었다. 시크릿 미설정 시엔 다른 통합과
/// 동일하게 503으로 비활성화(조용히 비활성화 원칙, doc/01 §1.9).
pub async fn webhook(
    Extension(pool): Extension<SiteScopedDb>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<serde_json::Value>, ApiError> {
    let secret = std::env::var("OXIPAGE_GITHUB_WEBHOOK_SECRET")
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "integration_disabled",
                "GitHub webhook requires OXIPAGE_GITHUB_WEBHOOK_SECRET",
            )
        })?;

    verify_signature(&secret, &headers, &body)?;

    let event: GithubEvent = serde_json::from_slice(&body)
        .map_err(|e| ApiError::validation("body", &format!("invalid GitHub event JSON: {e}")))?;
    validate_event(&event)?;
    repo::upsert(&pool.db, &event.into_input())
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(serde_json::json!({ "status": "ok" })))
}

/// `X-Hub-Signature-256: sha256=<hex>` 헤더를 HMAC-SHA256으로 검증한다.
/// Constant-time byte comparison (timing-attack safe).
fn ct_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

fn verify_signature(secret: &str, headers: &HeaderMap, body: &[u8]) -> Result<(), ApiError> {
    let provided = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "missing_signature",
                "missing X-Hub-Signature-256 header",
            )
        })?;

    let provided_hex = provided.strip_prefix("sha256=").ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_signature",
            "X-Hub-Signature-256 must start with 'sha256='",
        )
    })?;

    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| ApiError::internal(anyhow::anyhow!(e)))?;
    mac.update(body);
    let expected = mac.finalize().into_bytes();

    let provided_bytes = hex_decode(provided_hex).ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_signature",
            "signature is not valid hex",
        )
    })?;

    if !ct_eq(&provided_bytes, &expected) {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_signature",
            "signature verification failed",
        ));
    }
    Ok(())
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

pub async fn sync(Extension(pool): Extension<SiteScopedDb>) -> Result<Json<serde_json::Value>, ApiError> {
    let client = GithubClient::with_username(
        std::env::var("OXIPAGE_GITHUB_USERNAME").ok(),
    )
    .map_err(ApiError::internal)?;
    if !client.enabled() {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "integration_disabled",
            "GitHub activity sync requires OXIPAGE_GITHUB_USERNAME",
        ));
    }
    let events = client
        .fetch_public_events()
        .await
        .map_err(ApiError::internal)?;
    let count = events.len();
    for event in events {
        validate_event(&event)?;
        repo::upsert(&pool.db, &event.into_input())
            .await
            .map_err(ApiError::internal)?;
    }
    Ok(Json(serde_json::json!({ "status": "ok", "synced": count })))
}

fn validate_event(event: &GithubEvent) -> Result<(), ApiError> {
    if event.kind.trim().is_empty() {
        return Err(ApiError::validation("type", "event type must not be empty"));
    }
    if event.repo.name.trim().is_empty() {
        return Err(ApiError::validation(
            "repo.name",
            "repository name must not be empty",
        ));
    }
    if event.created_at.trim().is_empty() {
        return Err(ApiError::validation(
            "created_at",
            "occurred time must not be empty",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    fn sign(secret: &str, body: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(secret.as_bytes()).unwrap();
        mac.update(body);
        let bytes = mac.finalize().into_bytes();
        format!("sha256={}", hex_encode(&bytes))
    }

    fn hex_encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }

    fn header_with(sig: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("x-hub-signature-256", sig.parse().unwrap());
        h
    }

    #[test]
    fn accepts_valid_signature() {
        let body = br#"{"type":"PushEvent"}"#;
        let sig = sign("s3cret", body);
        assert!(verify_signature("s3cret", &header_with(&sig), body).is_ok());
    }

    #[test]
    fn rejects_wrong_secret() {
        let body = br#"{"type":"PushEvent"}"#;
        let sig = sign("s3cret", body);
        let err = verify_signature("other", &header_with(&sig), body).unwrap_err();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn rejects_tampered_body() {
        let sig = sign("s3cret", br#"{"type":"PushEvent"}"#);
        let err = verify_signature("s3cret", &header_with(&sig), br#"{"type":"DeleteEvent"}"#)
            .unwrap_err();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn rejects_missing_header() {
        let err = verify_signature("s3cret", &HeaderMap::new(), b"{}").unwrap_err();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn rejects_missing_prefix() {
        let mut h = HeaderMap::new();
        h.insert("x-hub-signature-256", "deadbeef".parse().unwrap());
        let err = verify_signature("s3cret", &h, b"{}").unwrap_err();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
    }

    #[test]
    fn rejects_non_hex() {
        let err = verify_signature("s3cret", &header_with("sha256=zzzz"), b"{}").unwrap_err();
        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
    }
}
