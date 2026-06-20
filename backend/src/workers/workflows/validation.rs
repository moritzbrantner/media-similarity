use std::collections::{BTreeMap, BTreeSet};

use super::{
    MediaFileKind, MediaWorkflowDocument, MediaWorkflowLibrary, MediaWorkflowNode,
    WorkflowDiagnostic,
};

pub fn validate_media_workflow_library(library: &MediaWorkflowLibrary) -> Vec<WorkflowDiagnostic> {
    let mut diagnostics = Vec::new();
    if library.format != super::LIBRARY_FORMAT {
        diagnostics.push(diagnostic(
            "invalid_format",
            format!(
                "Workflow library format must be `{}`",
                super::LIBRARY_FORMAT
            ),
            None,
            None,
            None,
        ));
    }
    if library.version != super::LIBRARY_VERSION {
        diagnostics.push(diagnostic(
            "invalid_version",
            format!(
                "Workflow library version must be {}",
                super::LIBRARY_VERSION
            ),
            None,
            None,
            None,
        ));
    }

    let documents = documents_by_id(library);
    for kind in MediaFileKind::ALL {
        let document_id = kind.as_str();
        let Some(entry) = documents.get(document_id) else {
            diagnostics.push(diagnostic(
                "missing_document",
                format!("Missing workflow document `{document_id}`"),
                Some(document_id),
                None,
                None,
            ));
            continue;
        };
        validate_document(kind, &entry.document, &mut diagnostics);
    }

    diagnostics
}

pub(crate) fn documents_by_id(
    library: &MediaWorkflowLibrary,
) -> BTreeMap<&str, &super::MediaWorkflowEntry> {
    library
        .documents
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect()
}

fn validate_document(
    kind: MediaFileKind,
    document: &MediaWorkflowDocument,
    diagnostics: &mut Vec<WorkflowDiagnostic>,
) {
    let document_id = kind.as_str();
    let mut node_ids = BTreeSet::new();
    let mut processors = BTreeSet::new();
    for node in &document.nodes {
        if !node_ids.insert(node.id.clone()) {
            diagnostics.push(diagnostic(
                "duplicate_node",
                format!(
                    "Workflow `{document_id}` contains duplicate node `{}`",
                    node.id
                ),
                Some(document_id),
                Some(&node.id),
                None,
            ));
        }
        let processor = node_processor(node);
        if !is_known_processor(processor) {
            diagnostics.push(diagnostic(
                "unknown_processor",
                format!("Workflow `{document_id}` contains unknown processor `{processor}`"),
                Some(document_id),
                Some(&node.id),
                None,
            ));
        }
        if node.data.enabled {
            processors.insert(processor.to_string());
            validate_config(
                document_id,
                &node.id,
                processor,
                &node.data.config,
                diagnostics,
            );
        }
    }
    for edge in &document.edges {
        if !node_ids.contains(&edge.source_node_id) || !node_ids.contains(&edge.target_node_id) {
            diagnostics.push(diagnostic(
                "missing_edge_node",
                format!("Workflow `{document_id}` contains an edge with a missing endpoint"),
                Some(document_id),
                None,
                Some(&edge.id),
            ));
        }
    }
    if has_cycle(document) {
        diagnostics.push(diagnostic(
            "cycle",
            format!("Workflow `{document_id}` must be acyclic"),
            Some(document_id),
            None,
            None,
        ));
    }
    for required in required_processors(kind) {
        if !processors.contains(required) {
            diagnostics.push(diagnostic(
                "missing_required_processor",
                format!("Workflow `{document_id}` is missing required processor `{required}`"),
                Some(document_id),
                None,
                None,
            ));
        }
    }
    if !path_starts_at_source(document) {
        diagnostics.push(diagnostic(
            "invalid_start",
            format!("Workflow `{document_id}` must start with `source.input`"),
            Some(document_id),
            None,
            None,
        ));
    }
    if !path_ends_at_qdrant(document) {
        diagnostics.push(diagnostic(
            "missing_index_sink",
            format!("Workflow `{document_id}` must end in `qdrant.upsert` for indexing"),
            Some(document_id),
            None,
            None,
        ));
    }
}

fn validate_config(
    document_id: &str,
    node_id: &str,
    processor: &str,
    config: &BTreeMap<String, serde_json::Value>,
    diagnostics: &mut Vec<WorkflowDiagnostic>,
) {
    let mut validator = ConfigValidator {
        document_id,
        node_id,
        processor,
        config,
        diagnostics,
    };
    match processor {
        "ocr.extract" => validator.validate_usize_config("ocr_max_frames", 1, 64),
        "faces.analyze" => {
            validator.validate_f32_config("face_detection_min_confidence", 0.0, 1.0);
            validator.validate_f32_config("face_cluster_threshold", 0.0, 2.0);
            validator.validate_u32_config("face_min_cluster_images", 1, u32::MAX);
            validator.validate_usize_config("face_max_frames_per_media", 1, usize::MAX);
        }
        "gif.decode" => {
            validator.validate_usize_config("gif_sample_frames", 1, usize::MAX);
            validator.validate_usize_config("gif_max_decode_frames", 1, usize::MAX);
            validator.validate_usize_config("gif_preview_frames", 1, usize::MAX);
            validator.validate_u32_config("gif_default_frame_delay_ms", 1, u32::MAX);
            validator.validate_f32_config("gif_motion_weight", 0.0, 1.0);
        }
        "video.detect_scenes" => {
            validator.validate_u32_config("video_frame_stride", 1, u32::MAX);
            validator.validate_optional_u32_config("video_max_frames", 1, u32::MAX);
        }
        "pdf.render_pages" => {
            validator.validate_u32_config("pdf_render_dpi", 72, 300);
            validator.validate_u32_config("pdf_max_pages", 1, 10_000);
            validator.validate_usize_config("pdf_summary_pages", 1, 256);
        }
        _ => {}
    }
}

fn required_processors(kind: MediaFileKind) -> Vec<&'static str> {
    let mut processors = vec![
        "source.input",
        "embedding.visual",
        "payload.build",
        "qdrant.upsert",
    ];
    match kind {
        MediaFileKind::StaticImage => processors.push("image.decode"),
        MediaFileKind::AnimatedGif => processors.push("gif.decode"),
        MediaFileKind::Video => processors.push("video.detect_scenes"),
        MediaFileKind::Audio => processors.push("audio.decode_segments"),
        MediaFileKind::Pdf => processors.push("pdf.render_pages"),
    }
    processors
}

struct ConfigValidator<'a> {
    document_id: &'a str,
    node_id: &'a str,
    processor: &'a str,
    config: &'a BTreeMap<String, serde_json::Value>,
    diagnostics: &'a mut Vec<WorkflowDiagnostic>,
}

impl ConfigValidator<'_> {
    fn validate_u32_config(&mut self, key: &str, min: u32, max: u32) {
        if let Some(value) = self.config.get(key) {
            let valid = value
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .map(|value| value >= min && value <= max)
                .unwrap_or(false);
            if !valid {
                self.push_invalid_config(key, min, max);
            }
        }
    }

    fn validate_optional_u32_config(&mut self, key: &str, min: u32, max: u32) {
        if self.config.get(key).is_some_and(serde_json::Value::is_null) {
            return;
        }
        self.validate_u32_config(key, min, max);
    }

    fn validate_usize_config(&mut self, key: &str, min: usize, max: usize) {
        if let Some(value) = self.config.get(key) {
            let valid = value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .map(|value| value >= min && value <= max)
                .unwrap_or(false);
            if !valid {
                self.push_invalid_config(key, min, max);
            }
        }
    }

    fn validate_f32_config(&mut self, key: &str, min: f32, max: f32) {
        if let Some(value) = self.config.get(key) {
            let valid = value
                .as_f64()
                .map(|value| {
                    let value = value as f32;
                    value.is_finite() && value >= min && value <= max
                })
                .unwrap_or(false);
            if !valid {
                self.push_invalid_config(key, min, max);
            }
        }
    }

    fn push_invalid_config(
        &mut self,
        key: &str,
        min: impl std::fmt::Display,
        max: impl std::fmt::Display,
    ) {
        self.diagnostics.push(invalid_config(
            self.document_id,
            self.node_id,
            self.processor,
            key,
            min,
            max,
        ));
    }
}

fn invalid_config(
    document_id: &str,
    node_id: &str,
    processor: &str,
    key: &str,
    min: impl std::fmt::Display,
    max: impl std::fmt::Display,
) -> WorkflowDiagnostic {
    diagnostic(
        "invalid_processor_config",
        format!("Processor `{processor}` config `{key}` must be between {min} and {max}"),
        Some(document_id),
        Some(node_id),
        None,
    )
}

fn diagnostic(
    code: impl Into<String>,
    message: impl Into<String>,
    document_id: Option<&str>,
    node_id: Option<&str>,
    edge_id: Option<&str>,
) -> WorkflowDiagnostic {
    WorkflowDiagnostic {
        code: code.into(),
        message: message.into(),
        document_id: document_id.map(ToOwned::to_owned),
        node_id: node_id.map(ToOwned::to_owned),
        edge_id: edge_id.map(ToOwned::to_owned),
    }
}

fn path_starts_at_source(document: &MediaWorkflowDocument) -> bool {
    let incoming = document
        .edges
        .iter()
        .map(|edge| edge.target_node_id.as_str())
        .collect::<BTreeSet<_>>();

    let roots = document
        .nodes
        .iter()
        .filter(|node| !incoming.contains(node.id.as_str()))
        .collect::<Vec<_>>();
    !roots.is_empty()
        && roots
            .iter()
            .all(|node| node_processor(node) == "source.input")
}

fn path_ends_at_qdrant(document: &MediaWorkflowDocument) -> bool {
    let outgoing = document
        .edges
        .iter()
        .map(|edge| edge.source_node_id.as_str())
        .collect::<BTreeSet<_>>();

    let terminals = document
        .nodes
        .iter()
        .filter(|node| !outgoing.contains(node.id.as_str()))
        .collect::<Vec<_>>();
    !terminals.is_empty()
        && terminals
            .iter()
            .all(|node| node_processor(node) == "qdrant.upsert")
}

fn has_cycle(document: &MediaWorkflowDocument) -> bool {
    let mut adjacency = BTreeMap::<&str, Vec<&str>>::new();
    for edge in &document.edges {
        adjacency
            .entry(edge.source_node_id.as_str())
            .or_default()
            .push(edge.target_node_id.as_str());
    }
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for node in &document.nodes {
        if visit_cycle(node.id.as_str(), &adjacency, &mut visiting, &mut visited) {
            return true;
        }
    }
    false
}

fn visit_cycle<'a>(
    node_id: &'a str,
    adjacency: &BTreeMap<&'a str, Vec<&'a str>>,
    visiting: &mut BTreeSet<&'a str>,
    visited: &mut BTreeSet<&'a str>,
) -> bool {
    if visited.contains(node_id) {
        return false;
    }
    if !visiting.insert(node_id) {
        return true;
    }
    for next in adjacency.get(node_id).into_iter().flatten() {
        if visit_cycle(next, adjacency, visiting, visited) {
            return true;
        }
    }
    visiting.remove(node_id);
    visited.insert(node_id);
    false
}

fn node_processor(node: &MediaWorkflowNode) -> &str {
    if node.data.processor.trim().is_empty() {
        node.kind.as_str()
    } else {
        node.data.processor.as_str()
    }
}

fn is_known_processor(processor: &str) -> bool {
    matches!(
        processor,
        "source.input"
            | "image.decode"
            | "gif.decode"
            | "video.detect_scenes"
            | "video.split_scenes"
            | "audio.decode_segments"
            | "pdf.render_pages"
            | "pdf.build_document_summary"
            | "photo.extract_metadata"
            | "ocr.extract"
            | "faces.analyze"
            | "audio.analyze"
            | "thumbnail.ensure"
            | "thumbnail.ensure_animated"
            | "embedding.visual"
            | "payload.build"
            | "qdrant.upsert"
    )
}
