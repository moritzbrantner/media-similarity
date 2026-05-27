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

    pub(super) fn conflict(detail: impl Into<String>) -> Self {
        Self {
            status: StatusCode::CONFLICT,
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
            JobError::InvalidArgument(message) | JobError::InvalidUri(message) => {
                Self::bad_request(message)
            }
            JobError::NotFound(message) => Self::not_found(message),
            JobError::Cancelled => Self::bad_request("job cancelled"),
            JobError::Failed(message)
            | JobError::Io(message)
            | JobError::StateUnavailable(message) => Self::internal(message),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(json!({ "detail": self.detail }))).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::ApiError;
    use axum::http::StatusCode;
    use jobs_core::JobError;

    #[test]
    fn job_errors_map_to_expected_http_statuses() {
        let cases = [
            (
                JobError::InvalidArgument("invalid input".to_string()),
                StatusCode::BAD_REQUEST,
            ),
            (
                JobError::InvalidUri("bad uri".to_string()),
                StatusCode::BAD_REQUEST,
            ),
            (JobError::Cancelled, StatusCode::BAD_REQUEST),
            (
                JobError::NotFound("missing job".to_string()),
                StatusCode::NOT_FOUND,
            ),
            (
                JobError::Io("disk full".to_string()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (
                JobError::Failed("job failed".to_string()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (
                JobError::StateUnavailable("lock poisoned".to_string()),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ];

        for (error, expected_status) in cases {
            assert_eq!(ApiError::from_job(error).status, expected_status);
        }
    }

    #[test]
    fn cancelled_job_error_has_stable_detail() {
        let error = ApiError::from_job(JobError::Cancelled);

        assert_eq!(error.status, StatusCode::BAD_REQUEST);
        assert_eq!(error.detail, "job cancelled");
    }
}
