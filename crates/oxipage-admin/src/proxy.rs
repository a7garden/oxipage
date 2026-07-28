//! 사이트 프록시 레이어.
//!
//! Admin SPA의 모든 사이트 API 호출은 이 프록시를 통해 이루어진다.
//! 토큰은 서버 측에서 주입 — 브라우저 JS에 노출되지 않는다.
//!
//! `ANY /api/admin/proxy/{site}/{*path}` → `{site.endpoint}/{path}` + `Authorization: Bearer {site.token}`

use crate::sites_api::SitesFile;
use crate::{AdminContext, AdminError};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, Method};
use axum::body::Bytes;
use axum::response::Response;

/// 사이트 프록시 핸들러.
pub(crate) async fn proxy_handler(
    State(ctx): State<AdminContext>,
    Path((site_name, path)): Path<(String, String)>,
    method: Method,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, AdminError> {
    // 1. sites.toml에서 사이트 조회
    let sf = SitesFile::load(&ctx.sites_path)?;
    let site = sf.sites.get(&site_name).ok_or_else(|| {
        AdminError::NotFound(format!("site '{site_name}' not found in sites.toml"))
    })?;

    // 2. 대상 URL 조립 — 경로 순회 방지
    if path.contains("..") || path.contains("//") {
        return Err(AdminError::BadRequest("invalid path".into()));
    }
    let base = site.endpoint.trim_end_matches('/');
    let target = if path.is_empty() {
        format!("{base}/")
    } else {
        format!("{base}/{path}")
    };

    // 3. 요청 빌드
    let mut req_builder = ctx.client.request(method.clone(), &target);

    // 본문이 있는 메서드만 body 전달
    if matches!(
        method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    ) {
        if !body.is_empty() {
            req_builder = req_builder.body(body.to_vec());
        }
    }

    // 4. 헤더 전달 (호스트/커넥션/Authority 등 제외)
    let hop_by_hop = [
        "host",
        "connection",
        "transfer-encoding",
        "upgrade",
        "proxy-authorization",
        "proxy-authenticate",
        "te",
        "trailer",
        "keep-alive",
    ];
    for (key, value) in headers.iter() {
        let key_str = key.as_str().to_lowercase();
        if !hop_by_hop.contains(&key_str.as_str()) {
            if let Ok(v) = value.to_str() {
                req_builder = req_builder.header(key.as_str(), v);
            }
        }
    }

    // 5. 인증 토큰 주입
    if let Some(token) = &site.token {
        req_builder = req_builder.header("Authorization", format!("Bearer {token}"));
    }

    // 6. 전송
    let response = req_builder.send().await.map_err(|e| {
        if e.is_timeout() {
            AdminError::Upstream(format!("upstream timeout after 30s: {site_name}"))
        } else if e.is_connect() {
            AdminError::Upstream(format!("cannot connect to {site_name} at {base}"))
        } else {
            AdminError::Upstream(format!("upstream error: {e}"))
        }
    })?;

    // 7. 응답 구성
    let status = response.status();
    let resp_headers = response.headers().clone();
    let resp_body = response.bytes().await.unwrap_or_default();

    let mut builder = Response::builder().status(status);
    // Content-Type, Content-Length 등 주요 헤더 전달
    for (key, value) in resp_headers.iter() {
        let key_str = key.as_str().to_lowercase();
        if key_str == "content-type"
            || key_str == "content-length"
            || key_str == "content-encoding"
            || key_str == "cache-control"
            || key_str == "etag"
            || key_str == "last-modified"
        {
            builder = builder.header(key.as_str(), value);
        }
    }

    builder
        .body(axum::body::Body::from(resp_body.to_vec()))
        .map_err(|e| AdminError::Internal(anyhow::anyhow!("response build failed: {e}")))
}
