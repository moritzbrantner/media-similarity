use std::path::{Path, PathBuf};
use std::str::FromStr;

use image_analysis_detection::FaceDetectionPreset;
use image_analysis_embeddings::{FaceEmbeddingPreset, ImageEmbeddingPreset};
use model_runtime::{
    HuggingFaceDownloader, HuggingFaceModelSpec, ModelBundle, ModelBundleFile, ModelBundleStore,
    ModelRuntimeError, ModelTask,
};
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
    pub blocking: bool,
    pub required_action: Option<&'static str>,
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

pub fn audio_transcription_spec(settings: &Settings) -> HuggingFaceModelSpec {
    audio_transcription_spec_for_model(&settings.audio_transcription_model)
}

pub fn audio_transcription_spec_for_model(model_id: &str) -> HuggingFaceModelSpec {
    HuggingFaceModelSpec::new(model_id, ModelTask::SpeechRecognition)
        .file("config.json")
        .file("generation_config.json")
        .file("tokenizer.json")
        .file("preprocessor_config.json")
        .file("model.safetensors")
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
    let spec = role_spec_for_settings(role, settings)?;
    let revision = spec.revision_value().unwrap_or("main");
    bundle_store(settings)
        .load(&spec.name, revision)
        .and_then(|bundle| normalize_onnx_role_bundle(bundle, role))
        .and_then(|bundle| validate_role_bundle(bundle, role))
        .map_err(|error| error.to_string())
}

pub fn download_role_bundle(role: ModelRole, settings: &Settings) -> Result<ModelBundle, String> {
    let spec = role_spec_for_settings(role, settings)?;
    if let Ok(bundle) = load_role_bundle(role, settings) {
        return Ok(bundle);
    }
    bundle_store(settings)
        .overwrite(true)
        .download(&spec)
        .and_then(|bundle| normalize_onnx_role_bundle(bundle, role))
        .and_then(|bundle| validate_role_bundle(bundle, role))
        .map_err(|error| error.to_string())
}

pub fn role_spec(role: ModelRole) -> Result<HuggingFaceModelSpec, String> {
    role_spec_for_settings(role, &Settings::default())
}

pub fn role_spec_for_settings(
    role: ModelRole,
    settings: &Settings,
) -> Result<HuggingFaceModelSpec, String> {
    match role {
        ModelRole::VisualEmbedding => Ok(visual_embedding_spec()),
        ModelRole::FaceDetection => Ok(face_detection_spec()),
        ModelRole::FaceEmbedding => Ok(face_embedding_spec()),
        ModelRole::AudioTranscription => Ok(audio_transcription_spec(settings)),
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
    let bundle = store
        .load(&spec.name, revision)
        .and_then(|bundle| normalize_onnx_role_bundle(bundle, role))
        .and_then(|bundle| validate_onnx_role_bundle(bundle, role));
    let bundle_error = match &bundle {
        Err(ModelRuntimeError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => Some(error.to_string()),
        Ok(_) => None,
    };
    let bundle = bundle.ok();
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
    } else if let Some(error) = bundle_error {
        Some(format!(
            "Model bundle `{}` is malformed or incomplete in {}: {error}",
            spec.name,
            settings.model_bundle_dir.display()
        ))
    } else if enabled {
        Some(format!(
            "Model bundle `{}` is not cached in {}",
            spec.name,
            settings.model_bundle_dir.display()
        ))
    } else {
        Some("Role is disabled by configuration".to_string())
    };
    let missing_enabled_model = enabled && !cached;

    ModelRuntimeStatus {
        role: role.as_str().to_string(),
        label: role.label().to_string(),
        configured: spec.name.clone(),
        cached,
        active: enabled && cached,
        blocking: role == ModelRole::VisualEmbedding && missing_enabled_model,
        required_action: missing_enabled_model.then_some("download"),
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
    let spec = audio_transcription_spec(settings);
    let store = bundle_store(settings);
    let revision = spec.revision_value().unwrap_or("main");
    let bundle = store
        .load(&spec.name, revision)
        .and_then(|bundle| validate_role_bundle(bundle, ModelRole::AudioTranscription));
    let bundle_error = match &bundle {
        Err(ModelRuntimeError::Io(error)) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => Some(error.to_string()),
        Ok(_) => None,
    };
    let bundle = bundle.ok();
    let cached = bundle.is_some();
    let missing_enabled_model = settings.audio_transcription_enabled && !cached;
    ModelRuntimeStatus {
        role: ModelRole::AudioTranscription.as_str().to_string(),
        label: ModelRole::AudioTranscription.label().to_string(),
        configured: spec.name.clone(),
        cached,
        active: settings.audio_transcription_enabled && cached,
        blocking: missing_enabled_model,
        required_action: missing_enabled_model.then_some("download"),
        bundle_path: bundle
            .as_ref()
            .map(|bundle| bundle.root.to_string_lossy().to_string()),
        detail: if cached {
            Some(format!("Using native ASR model bundle `{}`", spec.name))
        } else if let Some(error) = bundle_error {
            Some(format!(
                "Native ASR model bundle `{}` is malformed or incomplete in {}: {error}",
                spec.name,
                settings.model_bundle_dir.display()
            ))
        } else if settings.audio_transcription_enabled {
            Some(format!(
                "Native ASR model bundle `{}` is not cached in {}; download it before enabling transcription",
                spec.name,
                settings.model_bundle_dir.display()
            ))
        } else if !settings.audio_transcription_enabled {
            Some("Role is disabled by configuration".to_string())
        } else {
            None
        },
        options: vec![ModelOption {
            id: spec.name.clone(),
            label: spec.repo_id_value().unwrap_or(&spec.name).to_string(),
            cached,
            configured: true,
        }],
    }
}

fn fallback_path(path: &std::path::Path) -> Option<PathBuf> {
    path.is_file().then(|| path.to_path_buf())
}

fn normalize_onnx_role_bundle(
    mut bundle: ModelBundle,
    role: ModelRole,
) -> model_runtime::Result<ModelBundle> {
    if role == ModelRole::AudioTranscription {
        return Ok(bundle);
    }

    let task = onnx_role_task(role);
    let mut changed = false;
    if bundle.manifest.task != task {
        bundle.manifest.task = task;
        changed = true;
    }
    if ensure_default_face_config(&mut bundle, role)? {
        changed = true;
    }
    if changed {
        let encoded = serde_json::to_vec_pretty(&bundle.manifest).map_err(|error| {
            model_runtime::ModelRuntimeError::Source(format!(
                "failed to encode normalized model manifest: {error}"
            ))
        })?;
        std::fs::write(bundle.manifest_path(), encoded)?;
    }
    Ok(bundle)
}

fn ensure_default_face_config(
    bundle: &mut ModelBundle,
    role: ModelRole,
) -> model_runtime::Result<bool> {
    if !matches!(role, ModelRole::FaceDetection | ModelRole::FaceEmbedding) {
        return Ok(false);
    }

    let relative_path = bundle
        .manifest
        .files
        .get("config.json")
        .map(|file| PathBuf::from(&file.local_path))
        .unwrap_or_else(|| Path::new("files").join("config.json"));
    let path = bundle.root.join(&relative_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut changed = false;
    if !path.is_file() {
        std::fs::write(&path, b"{}\n")?;
        changed = true;
    }

    let size_bytes = std::fs::metadata(&path)?.len();
    match bundle.manifest.files.get("config.json") {
        Some(file) if file.size_bytes == size_bytes => Ok(changed),
        _ => {
            bundle.manifest.files.insert(
                "config.json".to_string(),
                ModelBundleFile {
                    remote_path: "config.json".to_string(),
                    local_path: path_to_manifest_string(&relative_path),
                    size_bytes,
                },
            );
            Ok(true)
        }
    }
}

fn validate_onnx_role_bundle(
    bundle: ModelBundle,
    role: ModelRole,
) -> model_runtime::Result<ModelBundle> {
    if role == ModelRole::AudioTranscription {
        return Ok(bundle);
    }

    for (remote_path, file) in &bundle.manifest.files {
        let local_path = bundle.root.join(&file.local_path);
        if !local_path.is_file() {
            return Err(ModelRuntimeError::Source(format!(
                "model bundle `{}` is missing cached file `{remote_path}` at {}",
                bundle.manifest.name,
                local_path.display()
            )));
        }
    }

    let onnx_count = bundle
        .manifest
        .files
        .values()
        .filter(|file| {
            Path::new(&file.remote_path)
                .extension()
                .and_then(|value| value.to_str())
                == Some("onnx")
        })
        .count();
    if onnx_count != 1 {
        return Err(ModelRuntimeError::Source(format!(
            "model bundle `{}` must contain exactly one `.onnx` model file, found {onnx_count}",
            bundle.manifest.name
        )));
    }

    Ok(bundle)
}

fn validate_role_bundle(
    bundle: ModelBundle,
    role: ModelRole,
) -> model_runtime::Result<ModelBundle> {
    if role == ModelRole::AudioTranscription {
        return validate_audio_transcription_bundle(bundle);
    }

    validate_onnx_role_bundle(bundle, role)
}

fn validate_audio_transcription_bundle(bundle: ModelBundle) -> model_runtime::Result<ModelBundle> {
    if bundle.manifest.task != ModelTask::SpeechRecognition {
        return Err(ModelRuntimeError::Source(format!(
            "native ASR model bundle `{}` must use speech_recognition task",
            bundle.manifest.name
        )));
    }
    for remote_path in [
        "config.json",
        "generation_config.json",
        "tokenizer.json",
        "preprocessor_config.json",
        "model.safetensors",
    ] {
        match bundle.file_path(remote_path) {
            Some(path) if path.is_file() => {}
            Some(path) => {
                return Err(ModelRuntimeError::Source(format!(
                    "native ASR model bundle `{}` is missing cached file `{remote_path}` at {}",
                    bundle.manifest.name,
                    path.display()
                )));
            }
            None => {
                return Err(ModelRuntimeError::Source(format!(
                    "native ASR model bundle `{}` is missing manifest entry `{remote_path}`",
                    bundle.manifest.name
                )));
            }
        }
    }
    Ok(bundle)
}

fn onnx_role_task(role: ModelRole) -> ModelTask {
    match role {
        ModelRole::VisualEmbedding => ModelTask::ImageEmbedding,
        ModelRole::FaceDetection => ModelTask::FaceDetection,
        ModelRole::FaceEmbedding => ModelTask::FaceEmbedding,
        ModelRole::AudioTranscription => {
            ModelTask::Custom(ModelRole::AudioTranscription.as_str().to_string())
        }
    }
}

fn path_to_manifest_string(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use model_runtime::{ModelBundle, ModelBundleFile, ModelBundleManifest, ModelTask};

    use super::{model_status, normalize_onnx_role_bundle, role_spec, ModelRole};
    use crate::config::Settings;

    #[test]
    fn missing_enabled_visual_model_is_blocking_and_downloadable() {
        let root =
            std::env::temp_dir().join(format!("image-sim-model-status-{}", std::process::id()));
        let settings = Settings {
            model_bundle_dir: root.join("bundles"),
            visual_embedding_enabled: true,
            visual_embedding_model_path: root.join("missing-model.onnx"),
            visual_embedding_preprocessor_path: root.join("missing-preprocessor.json"),
            ..Settings::default()
        };

        let status = model_status(ModelRole::VisualEmbedding, &settings);

        assert_eq!(status.role, "visual_embedding");
        assert!(!status.cached);
        assert!(!status.active);
        assert!(status.blocking);
        assert_eq!(status.required_action, Some("download"));
    }

    #[test]
    fn missing_enabled_audio_transcription_bundle_is_blocking_and_downloadable() {
        let root = std::env::temp_dir().join(format!(
            "audio-transcription-model-status-{}",
            std::process::id()
        ));
        let settings = Settings {
            model_bundle_dir: root.join("bundles"),
            audio_transcription_enabled: true,
            ..Settings::default()
        };

        let status = model_status(ModelRole::AudioTranscription, &settings);

        assert_eq!(status.role, "audio_transcription");
        assert_eq!(status.configured, "openai/whisper-large-v3-turbo");
        assert!(!status.cached);
        assert!(!status.active);
        assert!(status.blocking);
        assert_eq!(status.required_action, Some("download"));
        assert_eq!(status.options.len(), 1);
        assert_eq!(status.options[0].id, "openai/whisper-large-v3-turbo");
    }

    #[test]
    fn disabled_missing_visual_model_is_not_blocking() {
        let root = std::env::temp_dir().join(format!(
            "image-sim-model-status-disabled-{}",
            std::process::id()
        ));
        let settings = Settings {
            model_bundle_dir: root.join("bundles"),
            visual_embedding_enabled: false,
            visual_embedding_model_path: root.join("missing-model.onnx"),
            visual_embedding_preprocessor_path: root.join("missing-preprocessor.json"),
            ..Settings::default()
        };

        let status = model_status(ModelRole::VisualEmbedding, &settings);

        assert!(!status.cached);
        assert!(!status.active);
        assert!(!status.blocking);
        assert_eq!(status.required_action, None);
    }

    #[test]
    fn visual_embedding_role_uses_image_embedding_onnx_task() {
        let spec = role_spec(ModelRole::VisualEmbedding).expect("visual embedding spec");

        assert_eq!(spec.task, ModelTask::ImageEmbedding);
    }

    #[test]
    fn normalize_visual_bundle_repairs_legacy_role_task() {
        let root = std::env::temp_dir().join(format!(
            "image-sim-model-task-normalize-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("create bundle root");
        let manifest = ModelBundleManifest {
            schema_version: 1,
            name: "xenova-clip-vit-base-patch32-onnx".to_string(),
            repo_id: "Xenova/clip-vit-base-patch32".to_string(),
            revision: "main".to_string(),
            task: ModelTask::Custom("visual_embedding".to_string()),
            files: BTreeMap::new(),
        };
        std::fs::write(
            root.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).expect("encode manifest"),
        )
        .expect("write manifest");
        let bundle = ModelBundle { root, manifest };

        let bundle =
            normalize_onnx_role_bundle(bundle, ModelRole::VisualEmbedding).expect("normalize");

        assert_eq!(bundle.manifest.task, ModelTask::ImageEmbedding);
    }

    #[test]
    fn normalize_face_detection_bundle_adds_default_config() {
        let root = std::env::temp_dir().join(format!(
            "image-sim-face-config-normalize-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("files")).expect("create bundle files");
        std::fs::write(root.join("files/model.onnx"), b"model").expect("write model");
        let manifest = ModelBundleManifest {
            schema_version: 1,
            name: "opencv-yunet-onnx".to_string(),
            repo_id: "opencv/face_detection_yunet".to_string(),
            revision: "main".to_string(),
            task: ModelTask::FaceDetection,
            files: BTreeMap::from([(
                "face_detection_yunet_2023mar.onnx".to_string(),
                ModelBundleFile {
                    remote_path: "face_detection_yunet_2023mar.onnx".to_string(),
                    local_path: "files/model.onnx".to_string(),
                    size_bytes: 5,
                },
            )]),
        };
        let bundle = ModelBundle { root, manifest };

        let bundle =
            normalize_onnx_role_bundle(bundle, ModelRole::FaceDetection).expect("normalize");

        let config_path = bundle.file_path("config.json").expect("config path");
        assert!(config_path.is_file());
        assert_eq!(
            std::fs::read_to_string(config_path).expect("read config"),
            "{}\n"
        );
    }

    #[test]
    fn missing_bundle_file_is_not_reported_as_cached() {
        let root = std::env::temp_dir().join(format!(
            "image-sim-missing-bundle-file-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let bundle_root = root.join("bundles/opencv-yunet-onnx/main");
        std::fs::create_dir_all(bundle_root.join("files")).expect("create bundle files");
        std::fs::write(bundle_root.join("files/config.json"), b"{}").expect("write config");
        let manifest = ModelBundleManifest {
            schema_version: 1,
            name: "opencv-yunet-onnx".to_string(),
            repo_id: "opencv/face_detection_yunet".to_string(),
            revision: "main".to_string(),
            task: ModelTask::FaceDetection,
            files: BTreeMap::from([
                (
                    "config.json".to_string(),
                    ModelBundleFile {
                        remote_path: "config.json".to_string(),
                        local_path: "files/config.json".to_string(),
                        size_bytes: 2,
                    },
                ),
                (
                    "face_detection_yunet_2023mar.onnx".to_string(),
                    ModelBundleFile {
                        remote_path: "face_detection_yunet_2023mar.onnx".to_string(),
                        local_path: "files/missing.onnx".to_string(),
                        size_bytes: 5,
                    },
                ),
            ]),
        };
        std::fs::write(
            bundle_root.join("manifest.json"),
            serde_json::to_vec_pretty(&manifest).expect("encode manifest"),
        )
        .expect("write manifest");
        let settings = Settings {
            model_bundle_dir: root.join("bundles"),
            face_analysis_enabled: true,
            face_detection_model_path: root.join("missing-legacy-face-detector.onnx"),
            ..Settings::default()
        };

        let status = model_status(ModelRole::FaceDetection, &settings);

        assert!(!status.cached);
        assert!(!status.active);
        assert_eq!(status.required_action, Some("download"));
        assert!(status
            .detail
            .expect("detail")
            .contains("missing cached file"));
    }
}
