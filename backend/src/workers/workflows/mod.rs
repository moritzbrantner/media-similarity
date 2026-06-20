mod compiler;
mod defaults;
mod persistence;
mod types;
mod validation;

pub use compiler::{compile_media_workflow, media_workflow_hash};
pub use defaults::{
    default_media_workflow_library, media_workflow_node_templates, media_workflow_type_definitions,
};
pub use persistence::{
    load_media_workflow_library, save_media_workflow_library, workflow_file_is_writable,
};
pub use types::{
    CompiledMediaWorkflow, MediaFileKind, MediaWorkflowDocument, MediaWorkflowEdge,
    MediaWorkflowEntry, MediaWorkflowLibrary, MediaWorkflowNode, MediaWorkflowNodeData,
    MediaWorkflowNodeTemplate, MediaWorkflowPort, MediaWorkflowTypeDefinition, WorkflowDiagnostic,
    WorkflowMode, WorkflowViewport,
};
pub use validation::validate_media_workflow_library;

pub use types::{DEFAULT_CREATED_AT, LIBRARY_FORMAT, LIBRARY_VERSION};

#[cfg(test)]
mod tests {
    use super::{
        compile_media_workflow, default_media_workflow_library, load_media_workflow_library,
        media_workflow_hash, validate_media_workflow_library, MediaFileKind, WorkflowMode,
    };
    use crate::config::Settings;

    #[test]
    fn default_workflow_library_validates_and_compiles() {
        let library = default_media_workflow_library(&Settings::default());
        assert_eq!(validate_media_workflow_library(&library), Vec::new());
        let workflow =
            compile_media_workflow(MediaFileKind::StaticImage, WorkflowMode::Index, &library)
                .unwrap();
        assert!(workflow.processor_enabled("ocr.extract"));
        assert!(workflow.processor_enabled("qdrant.upsert"));
    }

    #[test]
    fn missing_required_document_fails_validation() {
        let mut library = default_media_workflow_library(&Settings::default());
        library.documents.retain(|entry| entry.id != "pdf");
        let diagnostics = validate_media_workflow_library(&library);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_document"));
    }

    #[test]
    fn missing_required_processor_fails_validation() {
        let mut library = default_media_workflow_library(&Settings::default());
        let document = &mut library.documents[0].document;
        document
            .nodes
            .retain(|node| node.data.processor != "embedding.visual");
        let diagnostics = validate_media_workflow_library(&library);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "missing_required_processor"));
    }

    #[test]
    fn disabled_optional_processor_compiles_and_is_skipped() {
        let mut library = default_media_workflow_library(&Settings::default());
        let node = library.documents[0]
            .document
            .nodes
            .iter_mut()
            .find(|node| node.data.processor == "ocr.extract")
            .unwrap();
        node.data.enabled = false;
        let diagnostics = validate_media_workflow_library(&library);
        assert_eq!(diagnostics, Vec::new());
        let workflow =
            compile_media_workflow(MediaFileKind::StaticImage, WorkflowMode::Index, &library)
                .unwrap();
        assert!(!workflow.processor_enabled("ocr.extract"));
    }

    #[test]
    fn cycles_fail_validation() {
        let mut library = default_media_workflow_library(&Settings::default());
        let document = &mut library.documents[0].document;
        document.edges.push(super::MediaWorkflowEdge {
            id: "cycle".to_string(),
            source_node_id: "qdrant-upsert".to_string(),
            source_port_id: "out".to_string(),
            target_node_id: "source-input".to_string(),
            target_port_id: "in".to_string(),
        });
        let diagnostics = validate_media_workflow_library(&library);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "cycle"));
    }

    #[test]
    fn unknown_processors_fail_validation() {
        let mut library = default_media_workflow_library(&Settings::default());
        library.documents[0].document.nodes[0].data.processor = "custom.processor".to_string();
        let diagnostics = validate_media_workflow_library(&library);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "unknown_processor"));
    }

    #[test]
    fn workflow_hash_changes_when_config_changes() {
        let mut library = default_media_workflow_library(&Settings::default());
        let before = media_workflow_hash(&library);
        library.documents[0].document.nodes[0].data.enabled = false;
        let after = media_workflow_hash(&library);
        assert_ne!(before, after);
    }

    #[test]
    fn invalid_numeric_config_reports_diagnostic() {
        let mut library = default_media_workflow_library(&Settings::default());
        let node = library.documents[0]
            .document
            .nodes
            .iter_mut()
            .find(|node| node.data.processor == "ocr.extract")
            .unwrap();
        node.data
            .config
            .insert("ocr_max_frames".to_string(), serde_json::json!(0));
        let diagnostics = validate_media_workflow_library(&library);
        assert!(diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "invalid_processor_config"));
    }

    #[test]
    fn missing_file_load_reports_error() {
        let path =
            std::env::temp_dir().join(format!("missing-workflows-{}.json", uuid::Uuid::new_v4()));
        assert!(load_media_workflow_library(&path).is_err());
    }
}
