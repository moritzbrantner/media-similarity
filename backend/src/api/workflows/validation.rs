use axum::Json;
use serde::{Deserialize, Serialize};

use crate::workers::workflows::{validate_media_workflow_library, WorkflowDiagnostic};

#[derive(Debug, Deserialize)]
pub struct ValidateWorkflowConfigRequest {
    pub library: crate::workers::workflows::MediaWorkflowLibrary,
}

#[derive(Debug, Serialize)]
pub struct ValidateWorkflowConfigResponse {
    pub diagnostics: Vec<WorkflowDiagnostic>,
}

pub async fn validate_workflows(
    Json(request): Json<ValidateWorkflowConfigRequest>,
) -> Json<ValidateWorkflowConfigResponse> {
    Json(ValidateWorkflowConfigResponse {
        diagnostics: validate_media_workflow_library(&request.library),
    })
}
