use std::path::PathBuf;
use std::str::FromStr;

use image_analysis_models::{FaceDetectionPreset, FaceEmbeddingPreset, ImageEmbeddingPreset};
use model_runtime::{HuggingFaceDownloader, HuggingFaceModelSpec, ModelBundle, ModelBundleStore};
use serde::Serialize;
use text_transcripts::{WhisperCppModel, WhisperCppModelStore};

use crate::config::Settings;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ModelRole {
    VisualEmbedding,
    FaceDetection,
    FaceEmbedding,
    AudioTranscription,
}

impl ModelRole {
    pub const ALL: [Self; 4] = [
        Self::VisualEmbedding,
        Self::FaceDetection,
        Self::FaceEmbedding,
        Self::AudioTranscription,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::VisualEmbedding => "visual_embedding",
            Self::FaceDetection => "face_detection",
            Self::FaceEmbedding => "face_embedding",
            Self::AudioTranscription => "audio_transcription",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::VisualEmbedding => "Visual embedding",
            Self::FaceDetection => "Face detection",
            Self::FaceEmbedding => "Face embedding",
            Self::AudioTranscription => "Audio transcription",
        }
    }
}

impl FromStr for ModelRole {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().replace('-', "_").as_str() {
            "visual_embedding" | "visual" => Ok(Self::VisualEmbedding),
            "face_detection" | "face_detector" => Ok(Self::FaceDetection),
            "face_embedding" | "face_embedder" => Ok(Self::FaceEmbedding),
            "audio_transcription" | "audio" | "transcription" | "whisper" => {
                Ok(Self::AudioTranscription)
            }
            other => Err(format!("Unknown model role `{other}`")),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ModelRuntimeStatus {
    pub role: String,
    pub label: String,
    pub configured: String,
    pub cached: bool,
    pub active: bool,
    pub bundle_path: Option<String>,
    pub detail: Option<String>,
    pub options: Vec<ModelOption>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ModelOption {
    pub id: String,
    pub label: String,
    pub cached: bool,
    pub configured: bool,
}

pub fn model_statuses(settings: &Settings) -> Vec<ModelRuntimeStatus> {
    ModelRole::ALL
        .into_iter()
        .map(|role| model_status(role, settings))
        .collect()
}

pub fn model_status(role: ModelRole, settings: &Settings) -> ModelRuntimeStatus {
    match role {
        ModelRole::VisualEmbedding => bundle_model_status(
            role,
            settings,
            visual_embedding_spec(),
            settings.visual_embedding_enabled,
            fallback_path(&settings.visual_embedding_model_path),
        ),
        ModelRole::FaceDetection => bundle_model_status(
            role,
            settings,
            face_detection_spec(),
            settings.face_analysis_enabled,
            fallback_path(&settings.face_detection_model_path),
        ),
        ModelRole::FaceEmbedding => bundle_model_status(
            role,
            settings,
            face_embedding_spec(),
            settings.face_analysis_enabled,
            fallback_path(&settings.face_embedding_model_path),
        ),
        ModelRole::AudioTranscription => audio_model_status(settings),
    }
}

pub fn visual_embedding_spec() -> HuggingFaceModelSpec {
    ImageEmbeddingPreset::XenovaClipVitBasePatch32Onnx.model_spec()
}

pub fn face_detection_spec() -> HuggingFaceModelSpec {
    FaceDetectionPreset::OpenCvYuNet.model_spec()
}

pub fn face_embedding_spec() -> HuggingFaceModelSpec {
    FaceEmbeddingPreset::OpenCvSFace.model_spec()
}

pub fn bundle_store(settings: &Settings) -> ModelBundleStore {
    ModelBundleStore::new(settings.model_bundle_dir.clone()).downloader(model_downloader(settings))
}

pub fn model_downloader(settings: &Settings) -> HuggingFaceDownloader {
    let mut downloader = HuggingFaceDownloader::new().progress(false).max_retries(2);
    if let Some(cache_dir) = &settings.model_hf_cache_dir {
        downloader = downloader.cache_dir(cache_dir.clone());
    }
    if let Some(token) = &settings.model_hf_token {
        downloader = downloader.token(token.clone());
    }
    downloader
}

pub fn load_role_bundle(role: ModelRole, settings: &Settings) -> Result<ModelBundle, String> {
    let spec = role_spec(role)?;
    let revision = spec.revision_value().unwrap_or("main");
    bundle_store(settings)
        .load(&spec.name, revision)
        .map_err(|error| error.to_string())
}

pub fn download_role_bundle(role: ModelRole, settings: &Settings) -> Result<ModelBundle, String> {
    let spec = role_spec(role)?;
    bundle_store(settings)
        .download(&spec)
        .map_err(|error| error.to_string())
}

pub fn role_spec(role: ModelRole) -> Result<HuggingFaceModelSpec, String> {
    match role {
        ModelRole::VisualEmbedding => Ok(visual_embedding_spec()),
        ModelRole::FaceDetection => Ok(face_detection_spec()),
        ModelRole::FaceEmbedding => Ok(face_embedding_spec()),
        ModelRole::AudioTranscription => {
            Err("audio transcription models are managed by text-transcripts".to_string())
        }
    }
}

pub fn parse_whisper_cpp_model(value: &str) -> Result<WhisperCppModel, String> {
    let normalized = value.trim();
    WhisperCppModel::ALL
        .into_iter()
        .find(|model| model.id().eq_ignore_ascii_case(normalized))
        .ok_or_else(|| format!("Unknown whisper.cpp model `{normalized}`"))
}

pub fn audio_transcription_model_store(settings: &Settings) -> WhisperCppModelStore {
    settings
        .audio_transcription_cache_dir
        .clone()
        .map(WhisperCppModelStore::new)
        .unwrap_or_default()
}

fn bundle_model_status(
    role: ModelRole,
    settings: &Settings,
    spec: HuggingFaceModelSpec,
    enabled: bool,
    fallback_path: Option<PathBuf>,
) -> ModelRuntimeStatus {
    let store = bundle_store(settings);
    let revision = spec.revision_value().unwrap_or("main");
    let bundle = store.load(&spec.name, revision).ok();
    let fallback_cached = fallback_path
        .as_ref()
        .map(|path| path.is_file())
        .unwrap_or(false);
    let cached = bundle.is_some() || fallback_cached;
    let bundle_path = bundle
        .as_ref()
        .map(|bundle| bundle.root.to_string_lossy().to_string())
        .or_else(|| fallback_path.map(|path| path.to_string_lossy().to_string()));
    let detail = if bundle.is_some() {
        Some(format!("Using model bundle `{}`", spec.name))
    } else if fallback_cached {
        Some("Using legacy path-based model configuration".to_string())
    } else if enabled {
        Some(format!(
            "Model bundle `{}` is not cached in {}",
            spec.name,
            settings.model_bundle_dir.display()
        ))
    } else {
        Some("Role is disabled by configuration".to_string())
    };

    ModelRuntimeStatus {
        role: role.as_str().to_string(),
        label: role.label().to_string(),
        configured: spec.name.clone(),
        cached,
        active: enabled && cached,
        bundle_path,
        detail,
        options: vec![ModelOption {
            id: spec.name.clone(),
            label: spec.repo_id_value().unwrap_or(&spec.name).to_string(),
            cached,
            configured: true,
        }],
    }
}

fn audio_model_status(settings: &Settings) -> ModelRuntimeStatus {
    let store = audio_transcription_model_store(settings);
    let configured = settings.audio_transcription_model.clone();
    let models = store
        .catalog()
        .models
        .into_iter()
        .map(|status| {
            let id = status.model.id().to_string();
            ModelOption {
                cached: status.cached,
                configured: id.eq_ignore_ascii_case(&configured),
                label: id.clone(),
                id,
            }
        })
        .collect::<Vec<_>>();
    let cached = models.iter().any(|model| model.configured && model.cached);
    ModelRuntimeStatus {
        role: ModelRole::AudioTranscription.as_str().to_string(),
        label: ModelRole::AudioTranscription.label().to_string(),
        configured,
        cached,
        active: settings.audio_transcription_enabled && cached,
        bundle_path: settings
            .audio_transcription_cache_dir
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        detail: if settings.audio_transcription_enabled && !cached {
            Some("Configured transcription model is not cached; native transcription will download it only when auto-download is enabled".to_string())
        } else if !settings.audio_transcription_enabled {
            Some("Role is disabled by configuration".to_string())
        } else {
            Some("Configured transcription model is cached".to_string())
        },
        options: models,
    }
}

fn fallback_path(path: &std::path::Path) -> Option<PathBuf> {
    path.is_file().then(|| path.to_path_buf())
}
