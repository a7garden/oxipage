//! HTTP 클라이언트 — 모든 명령의 공통 전송 계층 (doc/04 §4.1, §4.5).
//!
//! 응답 봉투:
//!   - 단건/목록: `{ "data": ... }`
//!   - 에러: `{ "error": { "code", "message", "field"? } }`

use anyhow::{Context, bail};
use serde::Serialize;
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, thiserror::Error)]
#[error("API error ({status} {code}): {message}{}",
        field.as_ref().map(|f| format!(" [field={f}]")).unwrap_or_default())]
pub struct ApiError {
    pub status: u16,
    pub code: String,
    pub message: String,
    pub field: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Client {
    endpoint: String,
    token: Option<String>,
    http: reqwest::Client,
}

impl Client {
    pub fn new(endpoint: String, token: Option<String>, insecure: bool) -> anyhow::Result<Self> {
        // endpoint trailing slash 정규화
        let endpoint = endpoint.trim_end_matches('/').to_string();
        let mut builder = reqwest::Client::builder()
            .user_agent(concat!("oxipage/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .pool_idle_timeout(Duration::from_secs(30));
        if insecure {
            builder = builder.danger_accept_invalid_certs(true);
        }
        let http = builder.build().context("failed to build HTTP client")?;
        Ok(Client {
            endpoint,
            token,
            http,
        })
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub fn has_token(&self) -> bool {
        self.token.is_some()
    }

    async fn request(
        &self,
        method: reqwest::Method,
        path: &str,
        body: Option<&Value>,
    ) -> Result<Value, ApiError> {
        let url = format!("{}{}", self.endpoint, path);
        let mut req = self.http.request(method, &url);
        if let Some(t) = &self.token {
            req = req.bearer_auth(t);
        }
        let req = if let Some(b) = body { req.json(b) } else { req };

        let resp = req.send().await.map_err(|e| ApiError {
            status: 0,
            code: "transport_error".into(),
            message: e.to_string(),
            field: None,
        })?;
        let status = resp.status();
        let status_num = status.as_u16();
        let text = resp.text().await.unwrap_or_default();
        let value: Value = serde_json::from_str(&text).unwrap_or_else(
            |_| serde_json::json!({ "error": { "code": "invalid_body", "message": text } }),
        );

        if !status.is_success() {
            let err = value.get("error").cloned().unwrap_or(value);
            return Err(ApiError {
                status: status_num,
                code: err
                    .get("code")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                message: err
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
                field: err
                    .get("field")
                    .and_then(|v| v.as_str())
                    .map(str::to_string),
            });
        }
        Ok(value)
    }

    pub async fn get(&self, path: &str) -> Result<Value, ApiError> {
        self.request(reqwest::Method::GET, path, None).await
    }

    pub async fn post<B: Serialize>(&self, path: &str, body: &B) -> Result<Value, ApiError> {
        let v = serde_json::to_value(body)
            .context("failed to serialize body")
            .map_err(|e| ApiError {
                status: 0,
                code: "serialize_error".into(),
                message: e.to_string(),
                field: None,
            })?;
        self.request(reqwest::Method::POST, path, Some(&v)).await
    }

    pub async fn post_raw(&self, path: &str, body: Value) -> Result<Value, ApiError> {
        self.request(reqwest::Method::POST, path, Some(&body)).await
    }

    pub async fn put<B: Serialize>(&self, path: &str, body: &B) -> Result<Value, ApiError> {
        let v = serde_json::to_value(body).map_err(|e| ApiError {
            status: 0,
            code: "serialize_error".into(),
            message: e.to_string(),
            field: None,
        })?;
        self.request(reqwest::Method::PUT, path, Some(&v)).await
    }

    pub async fn patch<B: Serialize>(&self, path: &str, body: &B) -> Result<Value, ApiError> {
        let v = serde_json::to_value(body).map_err(|e| ApiError {
            status: 0,
            code: "serialize_error".into(),
            message: e.to_string(),
            field: None,
        })?;
        self.request(reqwest::Method::PATCH, path, Some(&v)).await
    }

    pub async fn delete(&self, path: &str) -> Result<Value, ApiError> {
        self.request(reqwest::Method::DELETE, path, None).await
    }

    /// 응답에서 `data` 필드 추출. 단건 봉투 `{ "data": T }`.
    pub fn unwrap_data(value: Value) -> anyhow::Result<Value> {
        if let Some(d) = value.get("data") {
            Ok(d.clone())
        } else {
            bail!("missing `data` field in response: {value}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_endpoint_trimmed() {
        let client =
            Client::new("http://localhost:8787/".into(), Some("tok".into()), false).unwrap();
        assert_eq!(client.endpoint(), "http://localhost:8787");
    }

    #[test]
    fn test_client_has_token() {
        let client =
            Client::new("http://localhost:8787".into(), Some("tok".into()), false).unwrap();
        assert!(client.has_token());
    }

    #[test]
    fn test_client_no_token() {
        let client = Client::new("http://localhost:8787".into(), None, false).unwrap();
        assert!(!client.has_token());
    }

    #[test]
    fn test_api_error_display() {
        let err = ApiError {
            status: 404,
            code: "not_found".into(),
            message: "post not found".into(),
            field: Some("slug".into()),
        };
        let s = err.to_string();
        assert!(s.contains("404"));
        assert!(s.contains("not_found"));
        assert!(s.contains("post not found"));
        assert!(s.contains("field=slug"));
    }

    #[test]
    fn test_api_error_no_field() {
        let err = ApiError {
            status: 401,
            code: "unauthorized".into(),
            message: "bad token".into(),
            field: None,
        };
        let s = err.to_string();
        assert!(s.contains("401"));
        assert!(s.contains("unauthorized"));
        assert!(s.contains("bad token"));
        assert!(!s.contains("field="));
    }
}
