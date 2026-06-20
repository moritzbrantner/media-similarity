use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use super::io::{
    default_workflow_library, diagnostics_message, save_library, workflow_config_response,
};
use crate::api::{ApiError, AppState};
use crate::workers::workflows::{
    validate_media_workflow_library, MediaFileKind, MediaWorkflowLibrary,
    MediaWorkflowNodeTemplate, MediaWorkflowTypeDefinition, WorkflowDiagnostic,
};

#[derive(Debug, Serialize)]
pub struct WorkflowConfigResponse {
    pub workflow_file: String,
    pub writable: bool,
    pub library: MediaWorkflowLibrary,
    pub node_templates: Vec<MediaWorkflowNodeTemplate>,
    pub type_definitions: Vec<MediaWorkflowTypeDefinition>,
    pub diagnostics: Vec<WorkflowDiagnostic>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkflowConfigRequest {
    pub library: MediaWorkflowLibrary,
}

pub async fn get_workflows(State(state): State<Arc<AppState>>) -> Json<WorkflowConfigResponse> {
    Json(workflow_config_response(&state, state.workflow_library()))
}

pub async fn update_workflows(
    State(state): State<Arc<AppState>>,
    Json(request): Json<UpdateWorkflowConfigRequest>,
) -> Result<Json<WorkflowConfigResponse>, ApiError> {
    let diagnostics = validate_media_workflow_library(&request.library);
    if !diagnostics.is_empty() {
        return Err(ApiError::bad_request(diagnostics_message(&diagnostics)));
    }

    save_library(&state, &request.library)?;
    state.replace_workflow_library(request.library.clone());

    Ok(Json(workflow_config_response(&state, request.library)))
}

pub async fn reset_workflows(
    State(state): State<Arc<AppState>>,
) -> Result<Json<WorkflowConfigResponse>, ApiError> {
    let library = default_workflow_library(&state);
    save_library(&state, &library)?;
    state.replace_workflow_library(library.clone());
    Ok(Json(workflow_config_response(&state, library)))
}

#[allow(dead_code)]
fn _touch_media_file_kind(_: MediaFileKind) {}
