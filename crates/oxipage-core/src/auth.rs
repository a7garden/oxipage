use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::FromRequestParts;
use axum::http::StatusCode;
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;

/// v0 임시 인증 extractor. 쓰기 라우트의 핸들러 인자로 사용한다.
/// PAT 체계(doc §1.8)로 교체 예정 (Phase 1/4).
pub struct AdminAuth;

impl FromRequestParts<AppState> for AdminAuth {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Some(expected) = &state.admin_token else {
            return Err(ApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "admin_not_configured",
                "server has no OXIPAGE_ADMIN_TOKEN configured",
            ));
        };
        let provided = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "));
        match provided {
            Some(token) if constant_time_eq(token.as_bytes(), expected.as_bytes()) => Ok(AdminAuth),
            _ => Err(ApiError::new(
                StatusCode::UNAUTHORIZED,
                "unauthorized",
                "missing or invalid bearer token",
            )),
        }
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .fold(0u8, |acc, (x, y)| acc | (x ^ y))
        == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_time_eq_compares_bytes() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
        assert!(!constant_time_eq(b"", b"a"));
    }
}
