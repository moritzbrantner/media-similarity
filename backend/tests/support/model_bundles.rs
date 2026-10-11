//! Offline fixtures for model-bundle acceptance tests (#78).
//!
//! Bundles are fabricated on disk exactly the way `ModelBundleStore::materialize`
//! lays them out (`<root>/<safe name>/<safe revision>/manifest.json` plus
//! `files/<remote path>`), so no test needs a network download.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use model_runtime::{ModelBundleFile, ModelBundleManifest};
use sha2::{Digest, Sha256};

use image_similarity_service::config::Settings;
use image_similarity_service::workers::media::models::{role_spec_for_settings, ModelRole};

/// Unroutable Hugging Face endpoint: any download attempt fails fast with
/// "connection refused" instead of reaching the network.
pub const DENIED_HF_ENDPOINT: &str = "http://127.0.0.1:9";

/// Point the Hugging Face client at a denied endpoint and an isolated cache.
/// The values are identical for every caller, so concurrent tests do not race.
pub fn deny_model_network() {
    std::env::set_var("HF_ENDPOINT", DENIED_HF_ENDPOINT);
    std::env::set_var("HF_HUB_OFFLINE", "1");
    std::env::set_var(
        "HF_HOME",
        std::env::temp_dir().join("media-similarity-78-denied-hf-home"),
    );
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn safe_segment(value: &str) -> String {
    let safe: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-') {
                ch
            } else {
                '_'
            }
        })
        .collect();
    if safe.is_empty() {
        "_".to_string()
    } else {
        safe
    }
}

pub struct FabricatedBundle {
    pub root: PathBuf,
    pub repo_id: String,
    pub revision: String,
    /// remote path -> bytes written to the cached file
    pub files: BTreeMap<String, Vec<u8>>,
}

impl FabricatedBundle {
    pub fn local_file(&self, remote_path: &str) -> PathBuf {
        self.root.join("files").join(remote_path)
    }

    pub fn provenance_path(&self) -> PathBuf {
        self.root.join("provenance.json")
    }

    pub fn expected_source_url(&self, remote_path: &str) -> String {
        format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            self.repo_id, self.revision, remote_path
        )
    }
}

/// Write a cached bundle for `role` under `settings.model_bundle_dir`.
pub fn fabricate_role_bundle(
    role: ModelRole,
    settings: &Settings,
    files: &[(&str, &[u8])],
) -> FabricatedBundle {
    let spec = role_spec_for_settings(role, settings).expect("role spec");
    let repo_id = spec
        .repo_id_value()
        .expect("role spec is a Hugging Face spec")
        .to_string();
    let revision = spec.revision_value().unwrap_or("main").to_string();
    let root = settings
        .model_bundle_dir
        .join(safe_segment(&spec.name))
        .join(safe_segment(&revision));
    let mut manifest_files = BTreeMap::new();
    let mut written = BTreeMap::new();
    for (remote_path, bytes) in files {
        let local = Path::new("files").join(remote_path);
        let path = root.join(&local);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, bytes).unwrap();
        manifest_files.insert(
            remote_path.to_string(),
            ModelBundleFile {
                remote_path: remote_path.to_string(),
                local_path: format!("files/{remote_path}"),
                size_bytes: bytes.len() as u64,
            },
        );
        written.insert(remote_path.to_string(), bytes.to_vec());
    }
    let manifest = ModelBundleManifest {
        schema_version: 1,
        name: spec.name.clone(),
        repo_id: repo_id.clone(),
        revision: revision.clone(),
        task: spec.task.clone(),
        files: manifest_files,
    };
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    FabricatedBundle {
        root,
        repo_id,
        revision,
        files: written,
    }
}

/// Face detection bundle: one ONNX file plus the config the role normalizer expects.
pub fn fabricate_face_detection_bundle(settings: &Settings) -> FabricatedBundle {
    fabricate_role_bundle(
        ModelRole::FaceDetection,
        settings,
        &[
            ("config.json", b"abc"),
            (
                "face_detection_yunet_2023mar.onnx",
                b"fake yunet onnx bytes for provenance",
            ),
        ],
    )
}

/// Visual embedding bundle with a nested remote path.
pub fn fabricate_visual_embedding_bundle(settings: &Settings) -> FabricatedBundle {
    fabricate_role_bundle(
        ModelRole::VisualEmbedding,
        settings,
        &[
            ("config.json", b"{\"projection_dim\":512}"),
            ("onnx/model.onnx", b"fake clip onnx bytes"),
            ("preprocessor_config.json", b"{\"size\":224}"),
        ],
    )
}

/// Native ASR (candle-whisper) bundle with every file the role validator requires.
pub fn fabricate_audio_transcription_bundle(settings: &Settings) -> FabricatedBundle {
    fabricate_role_bundle(
        ModelRole::AudioTranscription,
        settings,
        &[
            ("config.json", b"{\"model_type\":\"whisper\"}"),
            ("generation_config.json", b"{}"),
            ("model.safetensors", b"fake safetensors weights"),
            ("preprocessor_config.json", b"{\"feature_size\":128}"),
            ("tokenizer.json", b"{\"version\":\"1.0\"}"),
        ],
    )
}

/// Expected `provenance.files` JSON, computed independently of the implementation.
pub fn expected_provenance_files(bundle: &FabricatedBundle) -> serde_json::Value {
    let mut entries: Vec<_> = bundle.files.iter().collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    serde_json::Value::Array(
        entries
            .into_iter()
            .map(|(remote_path, bytes)| {
                serde_json::json!({
                    "path": remote_path,
                    "sourceUrl": bundle.expected_source_url(remote_path),
                    "sha256": sha256_hex(bytes),
                    "sizeBytes": bytes.len() as u64,
                })
            })
            .collect(),
    )
}

/// Every regular file below `dir` (empty when `dir` does not exist).
pub fn files_below(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files_below(&path));
        } else {
            found.push(path);
        }
    }
    found
}
