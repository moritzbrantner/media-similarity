use super::config::WorkflowConfigResponse;
use crate::api::{ApiError, AppState};
use crate::workers::workflows::{
    default_media_workflow_library, media_workflow_node_templates, media_workflow_type_definitions,
    save_media_workflow_library, validate_media_workflow_library, workflow_file_is_writable,
    MediaWorkflowLibrary, WorkflowDiagnostic,
};

pub fn workflow_config_response(
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

pub fn diagnostics_message(diagnostics: &[WorkflowDiagnostic]) -> String {
    diagnostics
        .iter()
        .take(5)
        .map(|diagnostic| diagnostic.message.as_str())
        .collect::<Vec<_>>()
        .join("; ")
}

pub fn save_library(state: &AppState, library: &MediaWorkflowLibrary) -> Result<(), ApiError> {
    save_media_workflow_library(&state.settings.processing_workflows_file, library)
        .map_err(ApiError::internal)
}

pub fn default_workflow_library(state: &AppState) -> MediaWorkflowLibrary {
    default_media_workflow_library(&state.indexing_settings())
}
