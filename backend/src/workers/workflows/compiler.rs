use std::collections::BTreeMap;

use serde_json;
use sha2::{Digest, Sha256};

use super::validation::{documents_by_id, validate_media_workflow_library};
use super::{
    CompiledMediaWorkflow, MediaFileKind, MediaWorkflowLibrary, MediaWorkflowNode, WorkflowMode,
};

pub fn compile_media_workflow(
    kind: MediaFileKind,
    mode: WorkflowMode,
    library: &MediaWorkflowLibrary,
) -> Result<CompiledMediaWorkflow, String> {
    let diagnostics = validate_media_workflow_library(library);
    if !diagnostics.is_empty() {
        let message = diagnostics
            .iter()
            .take(3)
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>()
            .join("; ");
        return Err(message);
    }

    let documents = documents_by_id(library);
    let entry = documents
        .get(kind.as_str())
        .ok_or_else(|| format!("Missing workflow document `{}`", kind.as_str()))?;
    let mut processors = BTreeMap::new();
    for node in &entry.document.nodes {
        let processor = node_processor(node);
        if mode == WorkflowMode::Search && processor == "qdrant.upsert" {
            continue;
        }
        processors.insert(processor.to_string(), node.data.clone());
    }
    Ok(CompiledMediaWorkflow {
        kind,
        mode,
        processors,
    })
}

pub fn media_workflow_hash(library: &MediaWorkflowLibrary) -> String {
    let encoded = serde_json::to_vec(library).unwrap_or_default();
    format!("{:x}", Sha256::digest(encoded))
}

fn node_processor(node: &MediaWorkflowNode) -> &str {
    if node.data.processor.trim().is_empty() {
        node.kind.as_str()
    } else {
        node.data.processor.as_str()
    }
}
