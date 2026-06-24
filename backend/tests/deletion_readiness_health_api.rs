use std::fs;
use std::path::PathBuf;

use reqwest::StatusCode;
use serde_json::Value;
use url::form_urlencoded::byte_serialize;

use image_similarity_service::config::parse_extensions;

mod support;

use support::harness::TestApp;
use support::media_fixtures::write_pattern_image;

#[tokio::test]
async fn deleting_indexed_media_removes_records_and_generated_artifacts() {
    let app = focused_app(|_| {}).await;
    let source = app.source_path("delete-me.png");
    write_pattern_image(&source, 64, 40, [220, 20, 20], [20, 20, 20]);

    let indexed = app.index().await;
    assert_eq!(indexed.indexed, 1);
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);

    let payload = single_payload(&app);
    let thumbnail_path = generated_artifact_path(&app, payload.thumbnail_url.as_deref().unwrap());
    assert!(thumbnail_path.is_file());

    let before = app
        .search_upload("query.png", "image/png", fs::read(&source).unwrap(), None)
        .await;
    assert_eq!(before.count, 1);
    assert_eq!(before.results[0].image.id, payload.id);

    let deleted = app
        .delete_json(&format!("/api/indexed-media/{}", payload.id))
        .await;
    assert_eq!(deleted["deleted_points"], 1);
    assert_eq!(deleted["deleted_faces"], 0);
    assert_eq!(deleted["deleted_artifacts"], 1);
    assert!(deleted["errors"].as_array().unwrap().is_empty());

    let after = app
        .search_upload("query.png", "image/png", fs::read(&source).unwrap(), None)
        .await;
    assert_eq!(after.count, 0);
    assert!(app.stored_media_payloads().is_empty());
    assert_eq!(app.qdrant_operation_counts().deleted_points, 1);
    assert!(!thumbnail_path.exists());
}

#[tokio::test]
async fn deleting_indexed_source_removes_only_matching_records() {
    let app = focused_app(|settings| {
        let root = settings.source_image_dir.parent().unwrap().to_path_buf();
        let source_a = root.join("source-a");
        let source_b = root.join("source-b");
        fs::create_dir_all(&source_a).unwrap();
        fs::create_dir_all(&source_b).unwrap();
        settings.image_sources = vec![
            source_a.to_string_lossy().to_string(),
            source_b.to_string_lossy().to_string(),
        ];
    })
    .await;
    let source_a = app.root_path().join("source-a");
    let source_b = app.root_path().join("source-b");
    let deleted_file = source_a.join("delete-source.png");
    let kept_file = source_b.join("keep-source.png");
    write_pattern_image(&deleted_file, 64, 40, [220, 20, 20], [20, 20, 20]);
    write_pattern_image(&kept_file, 64, 40, [20, 180, 80], [20, 20, 20]);

    let indexed = app.index().await;
    assert_eq!(indexed.indexed, 2);
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);

    let delete_payload = payload_named(&app, "delete-source.png");
    let keep_payload = payload_named(&app, "keep-source.png");
    assert_ne!(delete_payload.source_uri, keep_payload.source_uri);

    let deleted = app
        .delete_json(&format!(
            "/api/indexed-sources?source_uri={}",
            query_value(delete_payload.source_uri.as_deref().unwrap())
        ))
        .await;
    assert_eq!(deleted["deleted_points"], 1);
    assert_eq!(deleted["errors"].as_array().unwrap().len(), 0);

    let payloads = app.stored_media_payloads();
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0].filename, "keep-source.png");

    let kept_search = app
        .search_upload(
            "query.png",
            "image/png",
            fs::read(&kept_file).unwrap(),
            Some(10),
        )
        .await;
    assert_eq!(kept_search.count, 1);
    assert_eq!(kept_search.results[0].image.filename, "keep-source.png");

    let deleted_search = app
        .search_upload(
            "query.png",
            "image/png",
            fs::read(&deleted_file).unwrap(),
            Some(10),
        )
        .await;
    assert_eq!(deleted_search.count, 1);
    assert_eq!(deleted_search.results[0].image.filename, "keep-source.png");
}

#[tokio::test]
async fn readiness_creates_required_qdrant_payload_indexes() {
    let app = focused_app(|_| {}).await;

    let response = app.raw_get("/api/ready").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], "ready");
    assert_eq!(check_named(&body, "qdrant")["status"], "ok");

    let schema = app.qdrant_payload_schema();
    for (field, data_type) in [
        ("point_kind", "keyword"),
        ("id", "keyword"),
        ("source_uri", "keyword"),
        ("source_item_uri", "keyword"),
        ("source_type", "keyword"),
        ("media_kind", "keyword"),
        ("photo_has_gps", "bool"),
        ("width", "integer"),
        ("height", "integer"),
        ("size_bytes", "integer"),
        ("modified_at", "float"),
        ("photo_capture_time_epoch", "float"),
    ] {
        assert_eq!(
            schema
                .get(field)
                .and_then(|value| value["data_type"].as_str()),
            Some(data_type),
            "payload index {field}"
        );
    }
}

#[tokio::test]
async fn readiness_reports_qdrant_dependency_failures_with_detail() {
    let app = focused_app(|settings| {
        settings.qdrant_url = "http://127.0.0.1:9".to_string();
        settings.qdrant_connect_timeout_ms = 100;
        settings.qdrant_request_timeout_ms = 100;
        settings.qdrant_retry_attempts = 0;
        settings.qdrant_retry_backoff_ms = 0;
    })
    .await;

    let response = app.raw_get("/api/ready").await;
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], "not_ready");
    let qdrant = check_named(&body, "qdrant");
    assert_eq!(qdrant["status"], "error");
    let detail = qdrant["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains("Qdrant") || detail.contains("qdrant") || detail.contains("127.0.0.1:9"),
        "{detail}"
    );
}

#[tokio::test]
async fn health_remains_liveness_only_when_qdrant_is_unavailable() {
    let app = focused_app(|settings| {
        settings.qdrant_url = "http://127.0.0.1:9".to_string();
        settings.qdrant_connect_timeout_ms = 100;
        settings.qdrant_request_timeout_ms = 100;
        settings.qdrant_retry_attempts = 0;
        settings.qdrant_retry_backoff_ms = 0;
    })
    .await;

    let response = app.raw_get("/api/health").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], "ok");
    assert_eq!(body["collection"], app.state.settings.qdrant_collection);
    assert_eq!(
        body["source_dir"],
        app.state
            .settings
            .source_image_dir
            .to_string_lossy()
            .to_string()
    );
}

async fn focused_app(
    configure: impl FnOnce(&mut image_similarity_service::config::Settings),
) -> TestApp {
    TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png").unwrap();
        settings.default_search_limit = 10;
        settings.duplicate_hash_distance = 0;
        settings.visual_embedding_enabled = false;
        settings.visual_embedding_backend = "legacy".to_string();
        settings.visual_embedding_vector_size = 32;
        settings.face_analysis_enabled = false;
        settings.ocr_enabled = false;
        configure(settings);
    })
    .await
}

fn single_payload(app: &TestApp) -> image_similarity_service::domain::models::ImagePayload {
    let payloads = app.stored_media_payloads();
    assert_eq!(payloads.len(), 1);
    payloads.into_iter().next().unwrap()
}

fn payload_named(
    app: &TestApp,
    filename: &str,
) -> image_similarity_service::domain::models::ImagePayload {
    app.stored_media_payloads()
        .into_iter()
        .find(|payload| payload.filename == filename)
        .unwrap_or_else(|| panic!("missing payload {filename}"))
}

fn generated_artifact_path(app: &TestApp, url: &str) -> PathBuf {
    let relative = url
        .strip_prefix("/thumbnails/")
        .unwrap_or_else(|| panic!("expected thumbnail URL, got {url}"));
    app.state.settings.thumbnail_dir.join(relative)
}

fn query_value(value: &str) -> String {
    byte_serialize(value.as_bytes()).collect()
}

fn check_named<'a>(body: &'a Value, name: &str) -> &'a Value {
    body["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"].as_str() == Some(name))
        .unwrap_or_else(|| panic!("missing readiness check {name}"))
}
