use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, serde::Serialize)]
pub struct ErrorBody {
    pub error: ErrorDetail,
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

    pub fn internal(err: anyhow::Error) -> Self {
        tracing::error!(error = ?err, "internal server error");
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "internal server error",
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}
