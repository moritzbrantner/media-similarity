use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use jobs_core::JobError;
use serde_json::json;

#[derive(Debug)]
pub struct ApiError {
    pub(crate) status: StatusCode,
    pub(crate) detail: String,
}

impl ApiError {
    pub(super) fn bad_request(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            detail: detail.into(),
        }
    }

    pub(super) fn payload_too_large(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::PAYLOAD_TOO_LARGE,
            detail: detail.into(),
        }
    }

    pub(super) fn not_found(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            detail: detail.into(),
        }
    }

    pub(super) fn service_unavailable(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::SERVICE_UNAVAILABLE,
            detail: detail.into(),
        }
    }

    pub(super) fn internal(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            detail: detail.into(),
        }
    }

    pub(super) fn from_job(error: JobError) -> Self {
        match error {
            JobError::InvalidArgument(message) => Self::bad_request(message),
            JobError::Cancelled => Self::bad_request("job cancelled"),
            JobError::Failed(message) | JobError::StateUnavailable(message) => {
                Self::internal(message)
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "detail": self.detail }))).into_response()
    }
}
