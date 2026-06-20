use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::config::Settings;

use super::{
    MediaFileKind, MediaWorkflowDocument, MediaWorkflowEdge, MediaWorkflowEntry,
    MediaWorkflowLibrary, MediaWorkflowNode, MediaWorkflowNodeData, MediaWorkflowNodeTemplate,
    MediaWorkflowPort, MediaWorkflowTypeDefinition, DEFAULT_CREATED_AT, LIBRARY_FORMAT,
    LIBRARY_VERSION,
};

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
            inputs: spec.inputs.iter().copied().map(input_port).collect(),
            outputs: spec.outputs.iter().copied().map(output_port).collect(),
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
        viewport: Some(super::WorkflowViewport {
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
