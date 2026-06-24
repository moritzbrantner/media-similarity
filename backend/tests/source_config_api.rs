use std::fs;

use reqwest::StatusCode;
use serde_json::{json, Value};

use image_similarity_service::config::parse_extensions;

mod support;

use support::harness::TestApp;
use support::media_fixtures::write_pattern_image;

#[tokio::test]
async fn source_config_read_reports_configured_sources_and_indexing_settings() {
    let app = source_config_app().await;
    let ready_source = app.source_path("ready-library");
    let missing_source = app.source_path("missing-library");
    fs::create_dir_all(&ready_source).unwrap();

    let updated = app
        .put_json(
            "/api/source-config",
            json!({
                "sources": [
                    ready_source.to_string_lossy(),
                    missing_source.to_string_lossy()
                ]
            }),
        )
        .await;
    assert_eq!(updated["sources"].as_array().unwrap().len(), 2);

    let response = app.get_json("/api/source-config").await;

    assert_eq!(
        response["media_sources_file"],
        app.media_sources_file().to_string_lossy().to_string()
    );
    assert_eq!(
        response["default_source_dir"],
        app.source_dir.to_string_lossy().to_string()
    );
    assert_eq!(response["media_sources_writable"], true);
    assert!(response["indexing"]["collection"].as_str().unwrap().len() > 5);
    assert_eq!(
        response["indexing"]["image_extensions"],
        json!([".jpg", ".png"])
    );
    assert_eq!(response["indexing"]["audio_extensions"], json!([".mp3"]));
    assert_eq!(response["indexing"]["pdf_extensions"], json!([".pdf"]));
    assert_eq!(
        response["indexing"]["video_extensions"],
        json!([".mp4", ".mov", ".m4v", ".webm", ".mkv", ".avi"])
    );

    let ready = source_by_spec(&response, &ready_source.to_string_lossy());
    assert_eq!(ready["kind"], "local");
    assert_eq!(ready["status"], "ready");
    assert!(ready["detail"].is_null());

    let missing = source_by_spec(&response, &missing_source.to_string_lossy());
    assert_eq!(missing["kind"], "local");
    assert_eq!(missing["status"], "unavailable");
    assert!(missing["detail"]
        .as_str()
        .is_some_and(|detail| detail.contains("Directory does not exist")));

    let supported_types = response["supported_source_types"].as_array().unwrap();
    assert!(supported_types
        .iter()
        .any(|entry| { entry["kind"] == "local" && entry["implemented"] == true }));
}

#[tokio::test]
async fn source_config_write_persists_sources_and_updates_runtime_indexing_roots() {
    let app = source_config_app().await;
    let library = app.source_path("configured-library");
    fs::create_dir_all(&library).unwrap();
    write_pattern_image(
        &library.join("configured.png"),
        40,
        40,
        [30, 180, 90],
        [20, 20, 20],
    );
    write_pattern_image(
        &app.source_path("default-only.png"),
        40,
        40,
        [180, 30, 90],
        [20, 20, 20],
    );

    let response = app
        .put_json(
            "/api/source-config",
            json!({ "sources": [format!("local://{}", library.display())] }),
        )
        .await;

    assert_eq!(
        response["sources"][0]["spec"],
        format!("local://{}", library.display())
    );
    assert_eq!(response["sources"][0]["status"], "ready");
    let persisted = fs::read_to_string(app.media_sources_file()).unwrap();
    assert!(persisted.contains("# Managed by image-similarity-service."));
    assert!(persisted.contains(&format!("local://{}", library.display())));

    let indexed = app.index().await;
    assert_eq!(indexed.indexed, 1);
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);
    assert!(indexed.errors.is_empty(), "{:?}", indexed.errors);
    assert_eq!(indexed.sources, vec![library.to_string_lossy().to_string()]);

    let payloads = app.stored_media_payloads();
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0].filename, "configured.png");
}

#[tokio::test]
async fn source_config_runtime_indexing_updates_affect_next_index_request() {
    let app = source_config_app().await;
    write_pattern_image(
        &app.source_path("allowed.png"),
        40,
        40,
        [30, 180, 90],
        [20, 20, 20],
    );
    write_pattern_image(
        &app.source_path("ignored.jpg"),
        40,
        40,
        [180, 30, 90],
        [20, 20, 20],
    );

    let mut indexing = app.get_json("/api/source-config").await["indexing"].clone();
    indexing["image_extensions"] = json!(["png"]);
    indexing["gif_sample_frames"] = json!(2);
    indexing["gif_motion_weight"] = json!(0.75);
    indexing["video_max_frames"] = json!(3);
    let response = app
        .put_json("/api/source-config", json!({ "indexing": indexing }))
        .await;

    assert_eq!(response["indexing"]["image_extensions"], json!([".png"]));
    assert_eq!(response["indexing"]["gif_sample_frames"], 2);
    assert_eq!(response["indexing"]["gif_motion_weight"], 0.75);
    assert_eq!(response["indexing"]["video_max_frames"], 3);

    let indexed = app.index().await;
    assert_eq!(indexed.indexed, 1);
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);
    assert!(indexed.errors.is_empty(), "{:?}", indexed.errors);
    let payloads = app.stored_media_payloads();
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0].filename, "allowed.png");
}

#[tokio::test]
async fn source_config_rejects_empty_source_updates_without_changing_runtime_sources() {
    let app = source_config_app().await;
    let configured = app.source_path("configured");
    fs::create_dir_all(&configured).unwrap();
    app.put_json(
        "/api/source-config",
        json!({ "sources": [configured.to_string_lossy()] }),
    )
    .await;

    let response = app
        .raw_put_json("/api/source-config", json!({ "sources": ["  ", "\n"] }))
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error: Value = response.json().await.unwrap();
    assert!(error["detail"]
        .as_str()
        .is_some_and(|detail| detail.contains("At least one media source")));

    let config = app.get_json("/api/source-config").await;
    assert_eq!(config["sources"].as_array().unwrap().len(), 1);
    assert_eq!(
        config["sources"][0]["spec"],
        configured.to_string_lossy().to_string()
    );
}

#[tokio::test]
async fn source_config_rejects_invalid_indexing_ranges_before_runtime_settings_change() {
    let app = source_config_app().await;
    let original = app.get_json("/api/source-config").await["indexing"].clone();
    let mut invalid = original.clone();
    invalid["face_detection_min_confidence"] = json!(1.5);

    let response = app
        .raw_put_json("/api/source-config", json!({ "indexing": invalid }))
        .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let error: Value = response.json().await.unwrap();
    assert!(error["detail"].as_str().is_some_and(|detail| {
        detail.contains("face_detection_min_confidence")
            && detail.contains("between 0")
            && detail.contains("1")
    }));

    let unchanged = app.get_json("/api/source-config").await;
    assert_eq!(unchanged["indexing"], original);
}

async fn source_config_app() -> TestApp {
    TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".jpg,.png").unwrap();
        settings.audio_extensions = parse_extensions(".mp3").unwrap();
        settings.pdf_extensions = parse_extensions(".pdf").unwrap();
        settings.duplicate_hash_distance = 0;
        settings.visual_embedding_backend = "legacy".to_string();
        settings.visual_embedding_vector_size = 32;
        settings.face_analysis_enabled = false;
        settings.ocr_enabled = false;
        settings.audio_transcription_enabled = false;
    })
    .await
}

fn source_by_spec<'a>(response: &'a Value, spec: &str) -> &'a Value {
    response["sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|source| source["spec"].as_str() == Some(spec))
        .unwrap()
}
