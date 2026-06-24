use std::time::Duration;

use reqwest::StatusCode;
use serde_json::{json, Value};

use image_similarity_service::config::parse_extensions;

mod support;

use support::harness::TestApp;
use support::media_fixtures::write_pattern_image;

#[tokio::test]
async fn index_job_snapshots_report_creation_progress_and_completion() {
    let app = indexing_job_app().await;
    write_pattern_image(
        &app.source_path("snapshot.png"),
        48,
        48,
        [220, 30, 30],
        [35, 35, 35],
    );

    let created = spawn_index_job(&app).await;
    let job_id = created["spec"]["id"].as_str().unwrap();

    assert_eq!(created["spec"]["kind"], "index.manual");
    assert_eq!(created["spec"]["name"], "Index media sources");
    assert_eq!(
        created["spec"]["metadata"]["collection"],
        app.state.settings.qdrant_collection
    );
    assert!(matches!(
        created["status"].as_str(),
        Some("Queued" | "Running" | "Succeeded")
    ));

    let finished = app.wait_for_job_status(job_id, &["Succeeded"]).await;
    assert_eq!(finished["metadata"]["needs_indexing"], "1");
    assert_eq!(finished["metadata"]["indexed"], "1");
    assert_eq!(finished["metadata"]["failed"], "0");
    assert_eq!(finished["progress"]["completed"], 1);
    assert_eq!(finished["progress"]["total"], 1);
    assert_eq!(finished["progress"]["unit"], "files");

    let jobs = app.get_json("/api/jobs").await;
    assert!(jobs
        .as_array()
        .unwrap()
        .iter()
        .any(|job| job["spec"]["id"] == job_id && job["status"] == "Succeeded"));
}

#[tokio::test]
async fn index_job_events_report_status_logs_progress_and_metadata() {
    let app = indexing_job_app().await;
    write_pattern_image(
        &app.source_path("events.png"),
        52,
        52,
        [30, 160, 80],
        [25, 25, 25],
    );

    let created = spawn_index_job(&app).await;
    let job_id = created["spec"]["id"].as_str().unwrap();
    app.wait_for_job_status(job_id, &["Succeeded"]).await;

    let events = app.get_json(&format!("/api/jobs/{job_id}/events")).await;
    let events = events.as_array().unwrap();

    assert!(events
        .iter()
        .any(|event| event["kind"]["StatusChanged"]["status"] == "Running"));
    assert!(events
        .iter()
        .any(|event| event["kind"]["StatusChanged"]["status"] == "Succeeded"));
    assert!(events.iter().any(|event| event["kind"]["Log"]["message"]
        .as_str()
        .is_some_and(|message| message.contains("checking indexed media sources"))));
    assert!(events
        .iter()
        .any(|event| event["kind"]["Progress"]["unit"] == "files"
            && event["kind"]["Progress"]["total"] == 1));
    assert!(
        events
            .iter()
            .any(|event| event["kind"]["Metadata"]
                == json!({ "key": "needs_indexing", "value": "1" }))
    );
    assert!(events
        .iter()
        .any(|event| event["kind"]["Metadata"] == json!({ "key": "indexed", "value": "1" })));
}

#[tokio::test]
async fn active_index_job_rejects_overlapping_index_requests() {
    let app = indexing_job_app().await;
    app.delay_qdrant_upserts(Duration::from_millis(500));
    write_pattern_image(
        &app.source_path("overlap.png"),
        60,
        60,
        [30, 80, 220],
        [20, 20, 20],
    );

    let created = spawn_index_job(&app).await;
    let job_id = created["spec"]["id"].as_str().unwrap();
    app.wait_for_job_status(job_id, &["Running"]).await;

    let background_response = app
        .client
        .post(format!("{}/api/jobs/index", app.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(background_response.status(), StatusCode::CONFLICT);
    let background_error: Value = background_response.json().await.unwrap();
    assert_eq!(
        background_error["detail"],
        "An indexing job is already running"
    );

    let sync_response = app
        .client
        .post(format!("{}/api/index", app.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(sync_response.status(), StatusCode::CONFLICT);
    let sync_error: Value = sync_response.json().await.unwrap();
    assert_eq!(sync_error["detail"], "An indexing job is already running");

    app.wait_for_job_status(job_id, &["Succeeded"]).await;
}

#[tokio::test]
async fn active_index_job_can_be_cancelled_through_public_job_route() {
    let app = indexing_job_app().await;
    app.delay_qdrant_upserts(Duration::from_millis(500));
    write_pattern_image(
        &app.source_path("cancel.png"),
        64,
        64,
        [180, 30, 90],
        [40, 40, 40],
    );

    let created = spawn_index_job(&app).await;
    let job_id = created["spec"]["id"].as_str().unwrap();
    app.wait_for_job_status(job_id, &["Running"]).await;

    let cancelling = app
        .client
        .post(format!("{}/api/jobs/{job_id}/cancel", app.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(cancelling.status(), StatusCode::OK);
    let cancelling: Value = cancelling.json().await.unwrap();
    assert_eq!(cancelling["status"], "Cancelling");

    let cancelled = app.wait_for_job_status(job_id, &["Cancelled"]).await;
    assert_eq!(cancelled["status"], "Cancelled");
    assert!(cancelled["finished_at"].is_string());

    let events = app.get_json(&format!("/api/jobs/{job_id}/events")).await;
    let events = events.as_array().unwrap();
    assert!(events
        .iter()
        .any(|event| event["kind"]["StatusChanged"]["status"] == "Cancelling"));
    assert!(events
        .iter()
        .any(|event| event["kind"]["StatusChanged"]["status"] == "Cancelled"));
}

async fn indexing_job_app() -> TestApp {
    TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png").unwrap();
        settings.duplicate_hash_distance = 0;
        settings.visual_embedding_backend = "legacy".to_string();
        settings.visual_embedding_vector_size = 32;
        settings.face_analysis_enabled = false;
        settings.ocr_enabled = false;
    })
    .await
}

async fn spawn_index_job(app: &TestApp) -> Value {
    let response = app
        .client
        .post(format!("{}/api/jobs/index", app.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response.json().await.unwrap()
}
