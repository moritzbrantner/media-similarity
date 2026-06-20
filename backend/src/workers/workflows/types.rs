use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::Settings;

pub const LIBRARY_FORMAT: &str = "@moritzbrantner/workflow-editor/library";
pub const LIBRARY_VERSION: u32 = 1;
pub const DEFAULT_CREATED_AT: &str = "2026-06-03T00:00:00.000Z";

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

    pub(crate) fn label(self) -> &'static str {
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
    pub(crate) processors: BTreeMap<String, MediaWorkflowNodeData>,
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

fn default_enabled() -> bool {
    true
}
