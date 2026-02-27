use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ErrorDetail {
    pub field: String,
    pub issue: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorBody {
    pub error: ErrorPayload,
}

#[derive(Debug, Clone, Serialize)]
pub struct ErrorPayload {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub details: Vec<ErrorDetail>,
    pub request_id: String,
}

#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    pub details: Vec<ErrorDetail>,
    pub request_id: String,
}

impl AppError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>, request_id: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            details: Vec::new(),
            request_id: request_id.into(),
        }
    }

    pub fn with_details(mut self, details: Vec<ErrorDetail>) -> Self {
        self.details = details;
        self
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let payload = ErrorBody {
            error: ErrorPayload {
                code: self.code,
                message: self.message,
                details: self.details,
                request_id: self.request_id,
            },
        };
        (self.status, Json(payload)).into_response()
    }
}
