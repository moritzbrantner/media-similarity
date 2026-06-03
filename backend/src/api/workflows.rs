use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use super::{ApiError, AppState};
use crate::workers::workflows::{
    default_media_workflow_library, media_workflow_node_templates, media_workflow_type_definitions,
    save_media_workflow_library, validate_media_workflow_library, workflow_file_is_writable,
    MediaWorkflowLibrary, MediaWorkflowNodeTemplate, MediaWorkflowTypeDefinition,
    WorkflowDiagnostic,
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

#[derive(Debug, Deserialize)]
pub struct ValidateWorkflowConfigRequest {
    pub library: MediaWorkflowLibrary,
}

#[derive(Debug, Serialize)]
pub struct ValidateWorkflowConfigResponse {
    pub diagnostics: Vec<WorkflowDiagnostic>,
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
    save_media_workflow_library(&state.settings.processing_workflows_file, &request.library)
        .map_err(ApiError::internal)?;
    state.replace_workflow_library(request.library.clone());
    Ok(Json(workflow_config_response(&state, request.library)))
}

pub async fn validate_workflows(
    Json(request): Json<ValidateWorkflowConfigRequest>,
) -> Json<ValidateWorkflowConfigResponse> {
    Json(ValidateWorkflowConfigResponse {
        diagnostics: validate_media_workflow_library(&request.library),
    })
}

pub async fn reset_workflows(
    State(state): State<Arc<AppState>>,
) -> Result<Json<WorkflowConfigResponse>, ApiError> {
    let library = default_media_workflow_library(&state.indexing_settings());
    save_media_workflow_library(&state.settings.processing_workflows_file, &library)
        .map_err(ApiError::internal)?;
    state.replace_workflow_library(library.clone());
    Ok(Json(workflow_config_response(&state, library)))
}

fn workflow_config_response(
    state: &AppState,
    library: MediaWorkflowLibrary,
) -> WorkflowConfigResponse {
    let settings = state.indexing_settings();
    WorkflowConfigResponse {
        workflow_file: settings
            .processing_workflows_file
            .to_string_lossy()
            .to_string(),
        writable: workflow_file_is_writable(&settings.processing_workflows_file),
        diagnostics: validate_media_workflow_library(&library),
        library,
        node_templates: media_workflow_node_templates(&settings),
        type_definitions: media_workflow_type_definitions(),
    }
}

fn diagnostics_message(diagnostics: &[WorkflowDiagnostic]) -> String {
    diagnostics
        .iter()
        .take(5)
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}
