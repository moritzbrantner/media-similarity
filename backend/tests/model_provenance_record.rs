//! Acceptance for #78 at the library seam: `record_model_provenance` writes
//! `<bundle root>/provenance.json` for an already-cached role bundle, and
//! `model_status` reads it back (fresh settings = restart) and flags cached
//! files whose bytes no longer match the recorded checksum.

use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use uuid::Uuid;

use image_similarity_service::config::Settings;
use image_similarity_service::workers::media::models::{
    model_status, record_model_provenance, ModelRole,
};

mod support;

use support::model_bundles::{
    deny_model_network, expected_provenance_files, fabricate_audio_transcription_bundle,
    fabricate_face_detection_bundle, fabricate_visual_embedding_bundle, files_below, sha256_hex,
};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("media-similarity-78-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn settings(&self) -> Settings {
        Settings {
            model_bundle_dir: self.0.join("model-bundles"),
            model_hf_cache_dir: Some(self.0.join("hf-cache")),
            visual_embedding_enabled: true,
            visual_embedding_model_path: self.0.join("missing-legacy-visual.onnx"),
            visual_embedding_preprocessor_path: self.0.join("missing-legacy-preprocessor.json"),
            face_analysis_enabled: true,
            face_detection_model_path: self.0.join("missing-legacy-face-detector.onnx"),
            face_embedding_model_path: self.0.join("missing-legacy-face-embedder.onnx"),
            audio_transcription_enabled: true,
            ..Settings::default()
        }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn json<T: serde::Serialize>(value: &T) -> Value {
    serde_json::to_value(value).expect("serialize")
}

#[test]
fn sha256_helper_matches_fips_test_vector() {
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn record_provenance_for_cached_visual_bundle_lists_source_url_checksum_and_size() {
    deny_model_network();
    let scratch = Scratch::new();
    let settings = scratch.settings();
    let bundle = fabricate_visual_embedding_bundle(&settings);

    let provenance = record_model_provenance(ModelRole::VisualEmbedding, &settings)
        .expect("record provenance for cached bundle");
    let provenance = json(&provenance);

    assert_eq!(provenance["role"], "visual_embedding");
    assert_eq!(provenance["modelId"], "Xenova/clip-vit-base-patch32");
    assert_eq!(provenance["modelId"], bundle.repo_id);
    assert_eq!(provenance["revision"], bundle.revision);
    assert_eq!(provenance["files"], expected_provenance_files(&bundle));
    let nested = provenance["files"]
        .as_array()
        .unwrap()
        .iter()
        .find(|file| file["path"] == "onnx/model.onnx")
        .expect("nested onnx file entry");
    assert_eq!(
        nested["sourceUrl"],
        "https://huggingface.co/Xenova/clip-vit-base-patch32/resolve/main/onnx/model.onnx"
    );
    assert_eq!(nested["sizeBytes"], b"fake clip onnx bytes".len() as u64);

    let on_disk: Value =
        serde_json::from_slice(&fs::read(bundle.provenance_path()).expect("provenance.json"))
            .expect("provenance.json is JSON");
    assert_eq!(on_disk, provenance);
}

#[test]
fn persisted_provenance_is_reported_by_status_after_restart() {
    deny_model_network();
    let scratch = Scratch::new();
    let bundle = fabricate_face_detection_bundle(&scratch.settings());
    let recorded = json(
        &record_model_provenance(ModelRole::FaceDetection, &scratch.settings())
            .expect("record provenance"),
    );

    // A fresh Settings value over the same model dir models a process restart.
    let status = json(&model_status(ModelRole::FaceDetection, &scratch.settings()));

    assert_eq!(status["cached"], true, "{status}");
    assert_eq!(status["provenance"], recorded);
    assert_eq!(
        status["provenance"]["files"],
        expected_provenance_files(&bundle)
    );
    let detail = status["detail"].as_str().unwrap_or_default().to_lowercase();
    assert!(
        !detail.contains("checksum"),
        "untouched bundle must not report a checksum problem: {detail}"
    );
}

#[test]
fn tampered_cached_file_is_not_reported_as_verified() {
    deny_model_network();
    let scratch = Scratch::new();
    let settings = scratch.settings();
    let bundle = fabricate_face_detection_bundle(&settings);
    let recorded = json(&record_model_provenance(ModelRole::FaceDetection, &settings).unwrap());
    let recorded_file = fs::read(bundle.provenance_path()).unwrap();

    // Same size, different bytes: only a checksum comparison can notice.
    let onnx = bundle.local_file("face_detection_yunet_2023mar.onnx");
    let original = fs::read(&onnx).unwrap();
    let tampered = vec![b'x'; original.len()];
    fs::write(&onnx, &tampered).unwrap();

    let status = json(&model_status(ModelRole::FaceDetection, &scratch.settings()));

    assert_eq!(
        status["provenance"], recorded,
        "status must keep reporting the recorded provenance, not re-hash silently"
    );
    assert_eq!(
        status["provenance"]["files"][1]["sha256"],
        sha256_hex(&original)
    );
    assert_eq!(
        status["cached"], true,
        "cached semantics stay as today (file present)"
    );
    let detail = status["detail"].as_str().unwrap_or_default().to_lowercase();
    assert!(
        detail.contains("checksum"),
        "status detail must flag the checksum mismatch: {status}"
    );
    assert!(
        detail.contains("face_detection_yunet_2023mar.onnx"),
        "status detail must name the mismatching file: {status}"
    );
    assert_eq!(
        fs::read(bundle.provenance_path()).unwrap(),
        recorded_file,
        "reading status must not rewrite provenance.json"
    );
}

#[test]
fn record_provenance_for_audio_transcription_bundle() {
    deny_model_network();
    let scratch = Scratch::new();
    let settings = scratch.settings();
    let bundle = fabricate_audio_transcription_bundle(&settings);

    let provenance = json(
        &record_model_provenance(ModelRole::AudioTranscription, &settings)
            .expect("record audio provenance"),
    );

    assert_eq!(provenance["role"], "audio_transcription");
    assert_eq!(provenance["modelId"], settings.audio_transcription_model);
    assert_eq!(provenance["files"], expected_provenance_files(&bundle));
    assert_eq!(provenance["files"].as_array().unwrap().len(), 5);
    let status = json(&model_status(
        ModelRole::AudioTranscription,
        &scratch.settings(),
    ));
    assert_eq!(status["provenance"], provenance);
}

#[test]
fn record_provenance_for_uncached_role_fails_without_downloading() {
    deny_model_network();
    let scratch = Scratch::new();
    let settings = scratch.settings();

    let result = record_model_provenance(ModelRole::FaceEmbedding, &settings);

    assert!(result.is_err(), "uncached role has nothing to record");
    assert!(files_below(&settings.model_bundle_dir).is_empty());
    assert!(files_below(settings.model_hf_cache_dir.as_ref().unwrap()).is_empty());
}
