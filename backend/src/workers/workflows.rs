use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::config::Settings;

const LIBRARY_FORMAT: &str = "@moritzbrantner/workflow-editor/library";
const LIBRARY_VERSION: u32 = 1;
const DEFAULT_CREATED_AT: &str = "2026-06-03T00:00:00.000Z";

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum MediaFileKind {
    StaticImage,
    AnimatedGif,
    Video,
    Audio,
    Pdf,
}

impl MediaFileKind {
    pub const ALL: [Self; 5] = [
        Self::StaticImage,
        Self::AnimatedGif,
        Self::Video,
        Self::Audio,
        Self::Pdf,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::StaticImage => "static_image",
            Self::AnimatedGif => "animated_gif",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Pdf => "pdf",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::StaticImage => "Static Image",
            Self::AnimatedGif => "Animated GIF",
            Self::Video => "Video",
            Self::Audio => "Audio",
            Self::Pdf => "PDF",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkflowMode {
    Index,
    Search,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MediaWorkflowLibrary {
    pub format: String,
    pub version: u32,
    #[serde(rename = "activeDocumentId")]
    pub active_document_id: Option<String>,
    pub documents: Vec<MediaWorkflowEntry>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MediaWorkflowEntry {
    pub id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
    pub version: u32,
    pub document: MediaWorkflowDocument,
    #[serde(default)]
    pub versions: Vec<Value>,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct MediaWorkflowDocument {
    pub nodes: Vec<MediaWorkflowNode>,
    pub edges: Vec<MediaWorkflowEdge>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport: Option<WorkflowViewport>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct WorkflowViewport {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MediaWorkflowNode {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    #[serde(
        default,
        rename = "categoryPath",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub category_path: Vec<String>,
    pub x: f64,
    pub y: f64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<MediaWorkflowPort>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<MediaWorkflowPort>,
    #[serde(default)]
    pub data: MediaWorkflowNodeData,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct MediaWorkflowNodeData {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub processor: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub config: BTreeMap<String, Value>,
    #[serde(default)]
    pub locked: bool,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MediaWorkflowPort {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub kind: String,
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub port_type: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MediaWorkflowEdge {
    pub id: String,
    #[serde(rename = "sourceNodeId")]
    pub source_node_id: String,
    #[serde(rename = "sourcePortId")]
    pub source_port_id: String,
    #[serde(rename = "targetNodeId")]
    pub target_node_id: String,
    #[serde(rename = "targetPortId")]
    pub target_port_id: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MediaWorkflowNodeTemplate {
    pub id: String,
    pub label: String,
    pub description: String,
    pub kind: String,
    #[serde(rename = "categoryPath")]
    pub category_path: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<MediaWorkflowPort>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub outputs: Vec<MediaWorkflowPort>,
    pub data: MediaWorkflowNodeData,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct MediaWorkflowTypeDefinition {
    pub name: String,
    #[serde(rename = "type")]
    pub type_definition: Value,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct WorkflowDiagnostic {
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edge_id: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CompiledMediaWorkflow {
    pub kind: MediaFileKind,
    pub mode: WorkflowMode,
    processors: BTreeMap<String, MediaWorkflowNodeData>,
}

impl CompiledMediaWorkflow {
    pub fn processor_enabled(&self, processor: &str) -> bool {
        self.processors
            .get(processor)
            .map(|data| data.enabled)
            .unwrap_or(false)
    }

    pub fn config_u32(&self, processor: &str, key: &str) -> Option<u32> {
        self.processors
            .get(processor)
            .and_then(|data| data.config.get(key))
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
    }

    pub fn config_usize(&self, processor: &str, key: &str) -> Option<usize> {
        self.processors
            .get(processor)
            .and_then(|data| data.config.get(key))
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
    }

    pub fn config_f32(&self, processor: &str, key: &str) -> Option<f32> {
        self.processors
            .get(processor)
            .and_then(|data| data.config.get(key))
            .and_then(Value::as_f64)
            .map(|value| value as f32)
            .filter(|value| value.is_finite())
    }

    pub fn apply_to_settings(&self, settings: &mut Settings) {
        settings.ocr_enabled = settings.ocr_enabled && self.processor_enabled("ocr.extract");
        settings.face_analysis_enabled =
            settings.face_analysis_enabled && self.processor_enabled("faces.analyze");
        if !self.processor_enabled("audio.analyze") {
            settings.audio_transcription_enabled = false;
        }

        if let Some(value) = self.config_usize("ocr.extract", "ocr_max_frames") {
            settings.ocr_max_frames = value;
        }
        if let Some(value) = self.config_f32("faces.analyze", "face_detection_min_confidence") {
            settings.face_detection_min_confidence = value;
        }
        if let Some(value) = self.config_f32("faces.analyze", "face_cluster_threshold") {
            settings.face_cluster_threshold = value;
        }
        if let Some(value) = self.config_u32("faces.analyze", "face_min_cluster_images") {
            settings.face_min_cluster_images = value;
        }
        if let Some(value) = self.config_usize("faces.analyze", "face_max_frames_per_media") {
            settings.face_max_frames_per_media = value;
        }
        if let Some(value) = self.config_usize("gif.decode", "gif_sample_frames") {
            settings.gif_sample_frames = value;
        }
        if let Some(value) = self.config_usize("gif.decode", "gif_max_decode_frames") {
            settings.gif_max_decode_frames = value;
        }
        if let Some(value) = self.config_usize("gif.decode", "gif_preview_frames") {
            settings.gif_preview_frames = value;
        }
        if let Some(value) = self.config_u32("gif.decode", "gif_default_frame_delay_ms") {
            settings.gif_default_frame_delay_ms = value;
        }
        if let Some(value) = self.config_f32("gif.decode", "gif_motion_weight") {
            settings.gif_motion_weight = value;
        }
        if let Some(value) = self.config_u32("video.detect_scenes", "video_frame_stride") {
            settings.video_frame_stride = value;
        }
        if let Some(value) = self.config_u32("video.detect_scenes", "video_max_frames") {
            settings.video_max_frames = Some(value);
        }
        if let Some(value) = self.config_u32("pdf.render_pages", "pdf_render_dpi") {
            settings.pdf_render_dpi = value;
        }
        if let Some(value) = self.config_u32("pdf.render_pages", "pdf_max_pages") {
            settings.pdf_max_pages = value;
        }
        if let Some(value) = self.config_usize("pdf.render_pages", "pdf_summary_pages") {
            settings.pdf_summary_pages = value;
        }
    }
}

pub fn default_media_workflow_library(settings: &Settings) -> MediaWorkflowLibrary {
    let documents = MediaFileKind::ALL
        .into_iter()
        .map(|kind| default_entry(kind, settings))
        .collect::<Vec<_>>();
    MediaWorkflowLibrary {
        format: LIBRARY_FORMAT.to_string(),
        version: LIBRARY_VERSION,
        active_document_id: Some(MediaFileKind::StaticImage.as_str().to_string()),
        documents,
    }
}

pub fn load_media_workflow_library(path: &Path) -> Result<MediaWorkflowLibrary, String> {
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    serde_json::from_str(&content)
        .map_err(|error| format!("Could not parse {}: {error}", path.display()))
}

pub fn save_media_workflow_library(
    path: &Path,
    library: &MediaWorkflowLibrary,
) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create workflow config directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let content = serde_json::to_string_pretty(library).map_err(|error| error.to_string())?;
    let temp = path.with_extension(format!(
        "{}.tmp",
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("json")
    ));
    fs::write(&temp, content)
        .map_err(|error| format!("Could not write {}: {error}", temp.display()))?;
    fs::rename(&temp, path).map_err(|error| {
        format!(
            "Could not move {} to {}: {error}",
            temp.display(),
            path.display()
        )
    })
}

pub fn workflow_file_is_writable(path: &Path) -> bool {
    if path.is_file() {
        return fs::OpenOptions::new().append(true).open(path).is_ok();
    }
    let Some(parent) = path.parent() else {
        return false;
    };
    if fs::create_dir_all(parent).is_err() {
        return false;
    }
    let probe = parent.join(format!(
        ".processing-workflows-writable-{}",
        std::process::id()
    ));
    match fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(probe);
            true
        }
        Err(_) => false,
    }
}

pub fn validate_media_workflow_library(library: &MediaWorkflowLibrary) -> Vec<WorkflowDiagnostic> {
    let mut diagnostics = Vec::new();
    if library.format != LIBRARY_FORMAT {
        diagnostics.push(diagnostic(
            "invalid_format",
            format!("Workflow library format must be `{LIBRARY_FORMAT}`"),
            None,
            None,
            None,
        ));
    }
    if library.version != LIBRARY_VERSION {
        diagnostics.push(diagnostic(
            "invalid_version",
            format!("Workflow library version must be {LIBRARY_VERSION}"),
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

pub fn media_workflow_node_templates(settings: &Settings) -> Vec<MediaWorkflowNodeTemplate> {
    processor_specs(settings)
        .into_iter()
        .map(|spec| MediaWorkflowNodeTemplate {
            id: format!("template-{}", spec.processor),
            label: spec.label.to_string(),
            description: spec.description.to_string(),
            kind: spec.processor.to_string(),
            category_path: spec
                .category_path
                .iter()
                .map(|value| value.to_string())
                .collect(),
            inputs: spec.inputs.into_iter().map(input_port).collect(),
            outputs: spec.outputs.into_iter().map(output_port).collect(),
            data: MediaWorkflowNodeData {
                processor: spec.processor.to_string(),
                enabled: true,
                config: spec.default_config,
                locked: spec.required,
            },
        })
        .collect()
}

pub fn media_workflow_type_definitions() -> Vec<MediaWorkflowTypeDefinition> {
    [
        "SourceFile",
        "DecodedImageMedia",
        "DecodedGifMedia",
        "VideoSceneSet",
        "AudioSegmentSet",
        "PdfPageSet",
        "PdfDocumentSummary",
        "AnalysisBundle",
        "PayloadSet",
        "VectorSet",
        "IndexedMediaSet",
    ]
    .into_iter()
    .map(|name| MediaWorkflowTypeDefinition {
        name: name.to_string(),
        type_definition: json!({ "kind": "object" }),
    })
    .collect()
}

fn default_entry(kind: MediaFileKind, settings: &Settings) -> MediaWorkflowEntry {
    MediaWorkflowEntry {
        id: kind.as_str().to_string(),
        name: kind.label().to_string(),
        description: Some(format!("Default {} processing workflow", kind.label())),
        tags: vec!["media-processing".to_string(), kind.as_str().to_string()],
        created_at: DEFAULT_CREATED_AT.to_string(),
        updated_at: DEFAULT_CREATED_AT.to_string(),
        version: 1,
        document: default_document(kind, settings),
        versions: Vec::new(),
    }
}

fn default_document(kind: MediaFileKind, settings: &Settings) -> MediaWorkflowDocument {
    let processors = match kind {
        MediaFileKind::StaticImage => vec![
            "source.input",
            "image.decode",
            "photo.extract_metadata",
            "ocr.extract",
            "faces.analyze",
            "thumbnail.ensure",
            "embedding.visual",
            "payload.build",
            "qdrant.upsert",
        ],
        MediaFileKind::AnimatedGif => vec![
            "source.input",
            "gif.decode",
            "ocr.extract",
            "faces.analyze",
            "thumbnail.ensure",
            "thumbnail.ensure_animated",
            "embedding.visual",
            "payload.build",
            "qdrant.upsert",
        ],
        MediaFileKind::Video => vec![
            "source.input",
            "video.detect_scenes",
            "video.split_scenes",
            "faces.analyze",
            "ocr.extract",
            "thumbnail.ensure",
            "embedding.visual",
            "payload.build",
            "qdrant.upsert",
        ],
        MediaFileKind::Audio => vec![
            "source.input",
            "audio.decode_segments",
            "audio.analyze",
            "thumbnail.ensure",
            "embedding.visual",
            "payload.build",
            "qdrant.upsert",
        ],
        MediaFileKind::Pdf => vec![
            "source.input",
            "pdf.render_pages",
            "ocr.extract",
            "pdf.build_document_summary",
            "thumbnail.ensure",
            "embedding.visual",
            "payload.build",
            "qdrant.upsert",
        ],
    };
    let specs = processor_specs(settings)
        .into_iter()
        .map(|spec| (spec.processor, spec))
        .collect::<BTreeMap<_, _>>();
    let nodes = processors
        .iter()
        .enumerate()
        .filter_map(|(index, processor)| {
            specs.get(processor).map(|spec| node_from_spec(index, spec))
        })
        .collect::<Vec<_>>();
    let edges = processors
        .windows(2)
        .map(|pair| MediaWorkflowEdge {
            id: format!("{}-{}", pair[0], pair[1]).replace('.', "-"),
            source_node_id: pair[0].replace('.', "-"),
            source_port_id: "out".to_string(),
            target_node_id: pair[1].replace('.', "-"),
            target_port_id: "in".to_string(),
        })
        .collect();
    MediaWorkflowDocument {
        nodes,
        edges,
        viewport: Some(WorkflowViewport {
            x: 40.0,
            y: 120.0,
            zoom: 0.85,
        }),
    }
}

fn node_from_spec(index: usize, spec: &ProcessorSpec) -> MediaWorkflowNode {
    MediaWorkflowNode {
        id: spec.processor.replace('.', "-"),
        label: spec.label.to_string(),
        description: Some(spec.description.to_string()),
        kind: spec.processor.to_string(),
        category: spec.category_path.first().map(|value| value.to_string()),
        category_path: spec
            .category_path
            .iter()
            .map(|value| value.to_string())
            .collect(),
        x: index as f64 * 280.0,
        y: 0.0,
        inputs: spec.inputs.iter().copied().map(input_port).collect(),
        outputs: spec.outputs.iter().copied().map(output_port).collect(),
        data: MediaWorkflowNodeData {
            processor: spec.processor.to_string(),
            enabled: true,
            config: spec.default_config.clone(),
            locked: spec.required,
        },
    }
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
    config: &BTreeMap<String, Value>,
    diagnostics: &mut Vec<WorkflowDiagnostic>,
) {
    match processor {
        "ocr.extract" => validate_usize_config(
            document_id,
            node_id,
            processor,
            config,
            "ocr_max_frames",
            1,
            64,
            diagnostics,
        ),
        "faces.analyze" => {
            validate_f32_config(
                document_id,
                node_id,
                processor,
                config,
                "face_detection_min_confidence",
                0.0,
                1.0,
                diagnostics,
            );
            validate_f32_config(
                document_id,
                node_id,
                processor,
                config,
                "face_cluster_threshold",
                0.0,
                2.0,
                diagnostics,
            );
            validate_u32_config(
                document_id,
                node_id,
                processor,
                config,
                "face_min_cluster_images",
                1,
                u32::MAX,
                diagnostics,
            );
            validate_usize_config(
                document_id,
                node_id,
                processor,
                config,
                "face_max_frames_per_media",
                1,
                usize::MAX,
                diagnostics,
            );
        }
        "gif.decode" => {
            validate_usize_config(
                document_id,
                node_id,
                processor,
                config,
                "gif_sample_frames",
                1,
                usize::MAX,
                diagnostics,
            );
            validate_usize_config(
                document_id,
                node_id,
                processor,
                config,
                "gif_max_decode_frames",
                1,
                usize::MAX,
                diagnostics,
            );
            validate_usize_config(
                document_id,
                node_id,
                processor,
                config,
                "gif_preview_frames",
                1,
                usize::MAX,
                diagnostics,
            );
            validate_u32_config(
                document_id,
                node_id,
                processor,
                config,
                "gif_default_frame_delay_ms",
                1,
                u32::MAX,
                diagnostics,
            );
            validate_f32_config(
                document_id,
                node_id,
                processor,
                config,
                "gif_motion_weight",
                0.0,
                1.0,
                diagnostics,
            );
        }
        "video.detect_scenes" => {
            validate_u32_config(
                document_id,
                node_id,
                processor,
                config,
                "video_frame_stride",
                1,
                u32::MAX,
                diagnostics,
            );
            validate_optional_u32_config(
                document_id,
                node_id,
                processor,
                config,
                "video_max_frames",
                1,
                u32::MAX,
                diagnostics,
            );
        }
        "pdf.render_pages" => {
            validate_u32_config(
                document_id,
                node_id,
                processor,
                config,
                "pdf_render_dpi",
                72,
                300,
                diagnostics,
            );
            validate_u32_config(
                document_id,
                node_id,
                processor,
                config,
                "pdf_max_pages",
                1,
                10_000,
                diagnostics,
            );
            validate_usize_config(
                document_id,
                node_id,
                processor,
                config,
                "pdf_summary_pages",
                1,
                256,
                diagnostics,
            );
        }
        _ => {}
    }
}

fn validate_u32_config(
    document_id: &str,
    node_id: &str,
    processor: &str,
    config: &BTreeMap<String, Value>,
    key: &str,
    min: u32,
    max: u32,
    diagnostics: &mut Vec<WorkflowDiagnostic>,
) {
    if let Some(value) = config.get(key) {
        let valid = value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .map(|value| value >= min && value <= max)
            .unwrap_or(false);
        if !valid {
            diagnostics.push(invalid_config(
                document_id,
                node_id,
                processor,
                key,
                min,
                max,
            ));
        }
    }
}

fn validate_optional_u32_config(
    document_id: &str,
    node_id: &str,
    processor: &str,
    config: &BTreeMap<String, Value>,
    key: &str,
    min: u32,
    max: u32,
    diagnostics: &mut Vec<WorkflowDiagnostic>,
) {
    if config.get(key).is_some_and(Value::is_null) {
        return;
    }
    validate_u32_config(
        document_id,
        node_id,
        processor,
        config,
        key,
        min,
        max,
        diagnostics,
    );
}

fn validate_usize_config(
    document_id: &str,
    node_id: &str,
    processor: &str,
    config: &BTreeMap<String, Value>,
    key: &str,
    min: usize,
    max: usize,
    diagnostics: &mut Vec<WorkflowDiagnostic>,
) {
    if let Some(value) = config.get(key) {
        let valid = value
            .as_u64()
            .and_then(|value| usize::try_from(value).ok())
            .map(|value| value >= min && value <= max)
            .unwrap_or(false);
        if !valid {
            diagnostics.push(invalid_config(
                document_id,
                node_id,
                processor,
                key,
                min,
                max,
            ));
        }
    }
}

fn validate_f32_config(
    document_id: &str,
    node_id: &str,
    processor: &str,
    config: &BTreeMap<String, Value>,
    key: &str,
    min: f32,
    max: f32,
    diagnostics: &mut Vec<WorkflowDiagnostic>,
) {
    if let Some(value) = config.get(key) {
        let valid = value
            .as_f64()
            .map(|value| {
                let value = value as f32;
                value.is_finite() && value >= min && value <= max
            })
            .unwrap_or(false);
        if !valid {
            diagnostics.push(invalid_config(
                document_id,
                node_id,
                processor,
                key,
                min,
                max,
            ));
        }
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

fn documents_by_id(library: &MediaWorkflowLibrary) -> BTreeMap<&str, &MediaWorkflowEntry> {
    library
        .documents
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect()
}

fn node_processor(node: &MediaWorkflowNode) -> &str {
    if node.data.processor.trim().is_empty() {
        node.kind.as_str()
    } else {
        node.data.processor.as_str()
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

fn processor_specs(settings: &Settings) -> Vec<ProcessorSpec> {
    vec![
        ProcessorSpec::new(
            "source.input",
            "Source input",
            "Reads a source file.",
            &["Input"],
            &[],
            &["SourceFile"],
            true,
        ),
        ProcessorSpec::new(
            "image.decode",
            "Decode image",
            "Decodes a static image.",
            &["Decode"],
            &["SourceFile"],
            &["DecodedImageMedia"],
            true,
        ),
        ProcessorSpec::new(
            "gif.decode",
            "Decode GIF",
            "Decodes and samples an animated GIF.",
            &["Decode"],
            &["SourceFile"],
            &["DecodedGifMedia"],
            true,
        )
        .config(gif_config(settings)),
        ProcessorSpec::new(
            "video.detect_scenes",
            "Detect video scenes",
            "Detects and samples video scenes.",
            &["Decode"],
            &["SourceFile"],
            &["VideoSceneSet"],
            true,
        )
        .config(video_config(settings)),
        ProcessorSpec::new(
            "video.split_scenes",
            "Expose scene clips",
            "Writes source scene clips for playback.",
            &["Artifacts"],
            &["VideoSceneSet"],
            &["VideoSceneSet"],
            false,
        ),
        ProcessorSpec::new(
            "audio.decode_segments",
            "Decode audio segments",
            "Renders audio windows as spectrogram media.",
            &["Decode"],
            &["SourceFile"],
            &["AudioSegmentSet"],
            true,
        ),
        ProcessorSpec::new(
            "pdf.render_pages",
            "Render PDF pages",
            "Renders PDF pages and reads embedded text.",
            &["Decode"],
            &["SourceFile"],
            &["PdfPageSet"],
            true,
        )
        .config(pdf_config(settings)),
        ProcessorSpec::new(
            "pdf.build_document_summary",
            "Build PDF summary",
            "Builds the whole-document PDF summary record.",
            &["Decode"],
            &["PdfPageSet"],
            &["PdfDocumentSummary"],
            false,
        ),
        ProcessorSpec::new(
            "photo.extract_metadata",
            "Photo metadata",
            "Extracts EXIF/IPTC photo metadata.",
            &["Analysis"],
            &["DecodedImageMedia"],
            &["AnalysisBundle"],
            false,
        ),
        ProcessorSpec::new(
            "ocr.extract",
            "OCR",
            "Extracts text from rendered media frames.",
            &["Analysis"],
            &["DecodedImageMedia"],
            &["AnalysisBundle"],
            false,
        )
        .config(ocr_config(settings)),
        ProcessorSpec::new(
            "faces.analyze",
            "Face analysis",
            "Detects and clusters faces.",
            &["Analysis"],
            &["DecodedImageMedia"],
            &["AnalysisBundle"],
            false,
        )
        .config(face_config(settings)),
        ProcessorSpec::new(
            "audio.analyze",
            "Audio analysis",
            "Analyzes speech, tempo, voices, and optional transcription.",
            &["Analysis"],
            &["AudioSegmentSet"],
            &["AnalysisBundle"],
            false,
        )
        .config(audio_config(settings)),
        ProcessorSpec::new(
            "thumbnail.ensure",
            "Thumbnail",
            "Writes the static thumbnail artifact.",
            &["Artifacts"],
            &["DecodedImageMedia"],
            &["DecodedImageMedia"],
            true,
        ),
        ProcessorSpec::new(
            "thumbnail.ensure_animated",
            "Animated thumbnail",
            "Writes an animated GIF preview artifact.",
            &["Artifacts"],
            &["DecodedGifMedia"],
            &["DecodedGifMedia"],
            false,
        ),
        ProcessorSpec::new(
            "embedding.visual",
            "Visual embedding",
            "Generates the visual search vector.",
            &["Embedding"],
            &["DecodedImageMedia"],
            &["VectorSet"],
            true,
        ),
        ProcessorSpec::new(
            "payload.build",
            "Build payload",
            "Builds Qdrant media payloads.",
            &["Payload"],
            &["VectorSet"],
            &["PayloadSet"],
            true,
        ),
        ProcessorSpec::new(
            "qdrant.upsert",
            "Upsert to Qdrant",
            "Stores payloads and vectors in Qdrant.",
            &["Storage"],
            &["PayloadSet"],
            &["IndexedMediaSet"],
            true,
        ),
    ]
}

#[derive(Clone)]
struct ProcessorSpec {
    processor: &'static str,
    label: &'static str,
    description: &'static str,
    category_path: &'static [&'static str],
    inputs: Vec<&'static str>,
    outputs: Vec<&'static str>,
    required: bool,
    default_config: BTreeMap<String, Value>,
}

impl ProcessorSpec {
    fn new(
        processor: &'static str,
        label: &'static str,
        description: &'static str,
        category_path: &'static [&'static str],
        inputs: &[&'static str],
        outputs: &[&'static str],
        required: bool,
    ) -> Self {
        Self {
            processor,
            label,
            description,
            category_path,
            inputs: inputs.to_vec(),
            outputs: outputs.to_vec(),
            required,
            default_config: BTreeMap::new(),
        }
    }

    fn config(mut self, config: BTreeMap<String, Value>) -> Self {
        self.default_config = config;
        self
    }
}

fn input_port(type_name: &str) -> MediaWorkflowPort {
    MediaWorkflowPort {
        id: "in".to_string(),
        label: type_name.to_string(),
        kind: type_name.to_string(),
        port_type: None,
    }
}

fn output_port(type_name: &str) -> MediaWorkflowPort {
    MediaWorkflowPort {
        id: "out".to_string(),
        label: type_name.to_string(),
        kind: type_name.to_string(),
        port_type: None,
    }
}

fn ocr_config(settings: &Settings) -> BTreeMap<String, Value> {
    BTreeMap::from([("ocr_max_frames".to_string(), json!(settings.ocr_max_frames))])
}

fn face_config(settings: &Settings) -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "face_detection_min_confidence".to_string(),
            json!(settings.face_detection_min_confidence),
        ),
        (
            "face_cluster_threshold".to_string(),
            json!(settings.face_cluster_threshold),
        ),
        (
            "face_min_cluster_images".to_string(),
            json!(settings.face_min_cluster_images),
        ),
        (
            "face_max_frames_per_media".to_string(),
            json!(settings.face_max_frames_per_media),
        ),
    ])
}

fn gif_config(settings: &Settings) -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "gif_sample_frames".to_string(),
            json!(settings.gif_sample_frames),
        ),
        (
            "gif_max_decode_frames".to_string(),
            json!(settings.gif_max_decode_frames),
        ),
        (
            "gif_preview_frames".to_string(),
            json!(settings.gif_preview_frames),
        ),
        (
            "gif_default_frame_delay_ms".to_string(),
            json!(settings.gif_default_frame_delay_ms),
        ),
        (
            "gif_motion_weight".to_string(),
            json!(settings.gif_motion_weight),
        ),
    ])
}

fn video_config(settings: &Settings) -> BTreeMap<String, Value> {
    BTreeMap::from([
        (
            "video_frame_stride".to_string(),
            json!(settings.video_frame_stride),
        ),
        (
            "video_max_frames".to_string(),
            json!(settings.video_max_frames),
        ),
    ])
}

fn pdf_config(settings: &Settings) -> BTreeMap<String, Value> {
    BTreeMap::from([
        ("pdf_render_dpi".to_string(), json!(settings.pdf_render_dpi)),
        ("pdf_max_pages".to_string(), json!(settings.pdf_max_pages)),
        (
            "pdf_summary_pages".to_string(),
            json!(settings.pdf_summary_pages),
        ),
    ])
}

fn audio_config(settings: &Settings) -> BTreeMap<String, Value> {
    BTreeMap::from([(
        "audio_transcription_enabled".to_string(),
        json!(settings.audio_transcription_enabled),
    )])
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

fn default_enabled() -> bool {
    true
}

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
