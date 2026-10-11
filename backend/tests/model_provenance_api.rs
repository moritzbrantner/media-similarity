//! Acceptance for #78 through the HTTP API: model bundles are fetched lazily
//! (status, readiness and indexing never download) and a role download leaves
//! provenance that `GET /api/models` reports, also after a restart.
//!
//! Every test denies model network access (`HF_ENDPOINT` points at a closed
//! local port) and fabricates cached bundles on disk, so none of them downloads.

use std::fs;

use serde_json::{json, Value};

use image_similarity_service::config::parse_extensions;

mod support;

use support::harness::TestApp;
use support::media_fixtures::write_pattern_image;
use support::model_bundles::{
    deny_model_network, expected_provenance_files, fabricate_audio_transcription_bundle,
    fabricate_face_detection_bundle, files_below,
};

fn model_entry<'a>(models: &'a Value, role: &str) -> &'a Value {
    models["models"]
        .as_array()
        .expect("models array")
        .iter()
        .find(|model| model["role"] == role)
        .unwrap_or_else(|| panic!("missing model role {role}"))
}

fn assert_provenance_field_present(entry: &Value) {
    assert!(
        entry
            .as_object()
            .expect("model entry object")
            .contains_key("provenance"),
        "GET /api/models entries must expose a `provenance` field (null when absent): {entry}"
    );
}

#[tokio::test]
async fn status_and_readiness_paths_never_download_model_bundles() {
    deny_model_network();
    let app = TestApp::new(|settings| {
        settings.visual_embedding_enabled = true;
        settings.visual_embedding_backend = "onnx".to_string();
        settings.face_analysis_enabled = true;
        settings.audio_transcription_enabled = true;
        let hf_cache = settings.model_bundle_dir.with_file_name("hf-cache");
        settings.model_hf_cache_dir = Some(hf_cache);
    })
    .await;
    let bundle_dir = app.state.settings.model_bundle_dir.clone();
    let hf_cache = app.state.settings.model_hf_cache_dir.clone().unwrap();

    let models = app.get_json("/api/models").await;
    let ready = app.raw_get("/api/ready").await;
    let _ = ready.text().await;
    let audio = app.get_json("/api/models/audio-transcription").await;
    let models_again = app.get_json("/api/models").await;

    assert_eq!(audio["models"][0]["cached"], false);
    assert!(
        files_below(&bundle_dir).is_empty(),
        "status/readiness created bundle files: {:?}",
        files_below(&bundle_dir)
    );
    assert!(
        files_below(&hf_cache).is_empty(),
        "status/readiness touched the Hugging Face cache: {:?}",
        files_below(&hf_cache)
    );
    assert_eq!(models["models"].as_array().unwrap().len(), 4);
    for role in [
        "visual_embedding",
        "face_detection",
        "face_embedding",
        "audio_transcription",
    ] {
        let entry = model_entry(&models_again, role);
        assert_eq!(entry["cached"], false, "{role} must stay uncached");
        assert_eq!(entry["required_action"], "download");
        assert_provenance_field_present(entry);
        assert!(
            entry["provenance"].is_null(),
            "uncached {role} has no provenance"
        );
    }
}

#[tokio::test]
async fn indexing_with_uncached_model_roles_does_not_download_them() {
    deny_model_network();
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png").unwrap();
        settings.visual_embedding_enabled = true;
        settings.visual_embedding_backend = "onnx".to_string();
        settings.face_analysis_enabled = true;
        settings.audio_transcription_enabled = false;
        let hf_cache = settings.model_bundle_dir.with_file_name("hf-cache");
        settings.model_hf_cache_dir = Some(hf_cache);
    })
    .await;
    write_pattern_image(
        &app.source_path("lazy-models.png"),
        48,
        48,
        [200, 30, 30],
        [20, 20, 20],
    );

    let started = app.post_json("/api/jobs/index", json!({})).await;
    let job_id = started["spec"]["id"].as_str().unwrap().to_string();
    app.wait_for_job_status(&job_id, &["Succeeded", "Failed"])
        .await;

    let bundle_dir = app.state.settings.model_bundle_dir.clone();
    let hf_cache = app.state.settings.model_hf_cache_dir.clone().unwrap();
    assert!(
        files_below(&bundle_dir).is_empty(),
        "indexing downloaded model bundles: {:?}",
        files_below(&bundle_dir)
    );
    assert!(
        files_below(&hf_cache).is_empty(),
        "indexing touched the Hugging Face cache: {:?}",
        files_below(&hf_cache)
    );
    let models = app.get_json("/api/models").await;
    for role in ["visual_embedding", "face_detection", "face_embedding"] {
        assert_eq!(model_entry(&models, role)["cached"], false);
    }
}

#[tokio::test]
async fn cached_bundle_without_record_reports_null_provenance() {
    deny_model_network();
    let app = TestApp::new(|settings| {
        settings.face_analysis_enabled = true;
    })
    .await;
    let bundle = fabricate_face_detection_bundle(&app.state.indexing_settings());

    let models = app.get_json("/api/models").await;
    let entry = model_entry(&models, "face_detection");

    assert_eq!(entry["cached"], true, "{entry}");
    assert_provenance_field_present(entry);
    assert!(entry["provenance"].is_null(), "{entry}");
    assert!(
        !bundle.provenance_path().exists(),
        "reading status must not write provenance.json"
    );
}

#[tokio::test]
async fn role_download_job_records_provenance_that_survives_restart() {
    deny_model_network();
    let app = TestApp::new(|settings| {
        settings.face_analysis_enabled = true;
    })
    .await;
    let bundle = fabricate_face_detection_bundle(&app.state.indexing_settings());

    let started = app
        .post_json("/api/models/face_detection/download", json!({}))
        .await;
    let job_id = started["spec"]["id"].as_str().unwrap().to_string();
    let finished = app
        .wait_for_job_status(&job_id, &["Succeeded", "Failed"])
        .await;
    assert_eq!(finished["status"], "Succeeded", "{finished}");

    let provenance_path = bundle.provenance_path();
    assert!(
        provenance_path.is_file(),
        "download must persist {}",
        provenance_path.display()
    );
    let models = app.get_json("/api/models").await;
    let entry = model_entry(&models, "face_detection");
    assert_eq!(entry["cached"], true);
    let provenance = &entry["provenance"];
    assert_eq!(provenance["role"], "face_detection", "{provenance}");
    assert_eq!(provenance["modelId"], bundle.repo_id);
    assert_eq!(provenance["revision"], bundle.revision);
    assert_eq!(provenance["files"], expected_provenance_files(&bundle));
    // Independent anchor: config.json holds "abc", whose SHA-256 is the FIPS 180-2 test vector.
    assert_eq!(
        provenance["files"][0]["sha256"],
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        provenance["files"][0]["sourceUrl"],
        format!(
            "https://huggingface.co/{}/resolve/{}/config.json",
            bundle.repo_id, bundle.revision
        )
    );

    // Other roles were not downloaded or recorded.
    for role in ["visual_embedding", "face_embedding", "audio_transcription"] {
        let other = model_entry(&models, role);
        assert_eq!(other["cached"], false);
        assert!(other["provenance"].is_null());
    }

    // "Restart": a fresh app state over the same model directory.
    let model_dir = app.state.settings.model_bundle_dir.clone();
    let restarted = TestApp::new(move |settings| {
        settings.face_analysis_enabled = true;
        settings.model_bundle_dir = model_dir;
    })
    .await;
    let after_restart = restarted.get_json("/api/models").await;
    assert_eq!(
        model_entry(&after_restart, "face_detection")["provenance"],
        *provenance
    );
    let on_disk: Value = serde_json::from_slice(&fs::read(&provenance_path).unwrap()).unwrap();
    assert_eq!(on_disk["files"], expected_provenance_files(&bundle));
}

#[tokio::test]
async fn audio_transcription_download_job_records_provenance() {
    deny_model_network();
    let app = TestApp::new(|settings| {
        settings.audio_transcription_enabled = false;
    })
    .await;
    let bundle = fabricate_audio_transcription_bundle(&app.state.indexing_settings());

    let started = app
        .post_json("/api/models/audio-transcription/download", json!({}))
        .await;
    let job_id = started["spec"]["id"].as_str().unwrap().to_string();
    let finished = app
        .wait_for_job_status(&job_id, &["Succeeded", "Failed"])
        .await;
    assert_eq!(finished["status"], "Succeeded", "{finished}");

    assert!(bundle.provenance_path().is_file());
    let models = app.get_json("/api/models").await;
    let provenance = &model_entry(&models, "audio_transcription")["provenance"];
    assert_eq!(provenance["role"], "audio_transcription", "{provenance}");
    assert_eq!(provenance["modelId"], "openai/whisper-large-v3-turbo");
    assert_eq!(provenance["revision"], "main");
    assert_eq!(provenance["files"], expected_provenance_files(&bundle));
    let weights_url =
        "https://huggingface.co/openai/whisper-large-v3-turbo/resolve/main/model.safetensors";
    assert!(provenance["files"]
        .as_array()
        .unwrap()
        .iter()
        .any(|file| file["sourceUrl"] == weights_url));
}
