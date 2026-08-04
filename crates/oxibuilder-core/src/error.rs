use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, serde::Serialize)]
pub struct ErrorBody {
    pub error: ErrorDetail,
    /// Optional payload attached to non-2xx responses that need to convey
    /// state alongside the error (e.g. the current remote row on 409).
    /// Skipped when `None` so existing handlers are unaffected.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, serde::Serialize)]
pub struct ErrorDetail {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
}

#[derive(Debug)]
pub struct ApiError {
    pub status: StatusCode,
    pub body: ErrorBody,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &str, message: &str) -> Self {
        ApiError {
            status,
            body: ErrorBody {
                error: ErrorDetail {
                    code: code.to_string(),
                    message: message.to_string(),
                    field: None,
                },
                data: None,
            },
        }
    }

    pub fn validation(field: &str, message: &str) -> Self {
        let mut err = ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation_error",
            message,
        );
        err.body.error.field = Some(field.to_string());
        err
    }

    /// Build an error response that also carries `data` (any `Serialize`).
    /// The JSON body becomes `{ error: {...}, data: <value> }`.
    pub fn with_data<T: serde::Serialize>(
        status: StatusCode,
        code: &str,
        message: &str,
        data: &T,
    ) -> Self {
        let value = serde_json::to_value(data).ok();
        ApiError {
            status,
            body: ErrorBody {
                error: ErrorDetail {
                    code: code.to_string(),
                    message: message.to_string(),
                    field: None,
                },
                data: value,
            },
        }
    }

    pub fn internal(err: anyhow::Error) -> Self {
        tracing::error!(error = ?err, "internal server error");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "internal server error",
        )
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        Self::internal(err)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}
