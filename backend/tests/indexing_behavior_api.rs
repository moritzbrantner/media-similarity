use std::fs;
use std::time::Duration;

use serde_json::json;

use image_similarity_service::config::parse_extensions;

mod support;

use support::harness::TestApp;
use support::media_fixtures::write_pattern_image;

#[tokio::test]
async fn repeat_indexing_current_local_file_does_not_replace_media_point() {
    let app = indexing_app().await;
    let image = app.source_path("current.png");
    write_pattern_image(&image, 48, 48, [220, 30, 30], [35, 35, 35]);

    let first = app.index().await;
    assert_eq!(first.indexed, 1);
    assert_eq!(first.already_indexed, 0);
    assert_eq!(first.pruned, 0);
    assert!(first.errors.is_empty(), "{:?}", first.errors);
    let counts_after_first = app.qdrant_operation_counts();

    let second = app.index().await;
    assert_eq!(second.indexed, 0);
    assert_eq!(second.already_indexed, 1);
    assert_eq!(second.pruned, 0);
    assert_eq!(second.failed, 0, "{:?}", second.errors);
    assert!(second.errors.is_empty(), "{:?}", second.errors);
    assert_eq!(app.stored_media_payloads().len(), 1);
    assert_eq!(
        app.qdrant_operation_counts().upserted_points,
        counts_after_first.upserted_points
    );
    assert_eq!(
        app.qdrant_operation_counts().deleted_points,
        counts_after_first.deleted_points
    );
}

#[tokio::test]
async fn modified_local_file_replaces_previous_payload_for_source_item() {
    let app = indexing_app().await;
    let image = app.source_path("changed.png");
    write_pattern_image(&image, 40, 40, [30, 160, 80], [25, 25, 25]);

    let first = app.index().await;
    assert_eq!(first.indexed, 1);
    let first_payload = single_payload(&app);

    tokio::time::sleep(Duration::from_millis(1100)).await;
    write_pattern_image(&image, 64, 52, [30, 160, 80], [230, 230, 40]);

    let second = app.index().await;
    assert_eq!(second.indexed, 1);
    assert_eq!(second.already_indexed, 0);
    assert_eq!(second.pruned, 0);
    assert_eq!(second.failed, 0, "{:?}", second.errors);
    assert!(second.errors.is_empty(), "{:?}", second.errors);

    let replacement = single_payload(&app);
    assert_eq!(replacement.id, first_payload.id);
    assert_eq!(replacement.source_item_uri, first_payload.source_item_uri);
    assert_eq!(replacement.filename, "changed.png");
    assert_eq!(replacement.width, 64);
    assert_eq!(replacement.height, 52);
    assert_ne!(replacement.size_bytes, first_payload.size_bytes);
    assert!(replacement.modified_at > first_payload.modified_at);
}

#[tokio::test]
async fn removed_local_file_prunes_stale_media_point() {
    let app = indexing_app().await;
    let image = app.source_path("removed.png");
    write_pattern_image(&image, 44, 44, [30, 80, 220], [20, 20, 20]);

    let first = app.index().await;
    assert_eq!(first.indexed, 1);
    assert_eq!(app.stored_media_payloads().len(), 1);

    fs::remove_file(&image).unwrap();

    let second = app.index().await;
    assert_eq!(second.indexed, 0);
    assert_eq!(second.already_indexed, 0);
    assert_eq!(second.pruned, 1);
    assert_eq!(second.failed, 0, "{:?}", second.errors);
    assert!(second.errors.is_empty(), "{:?}", second.errors);
    assert!(app.stored_media_payloads().is_empty());
    assert_eq!(app.qdrant_operation_counts().deleted_points, 1);
}

#[tokio::test]
async fn runtime_image_extension_config_controls_indexed_local_files() {
    let app = indexing_app().await;
    write_pattern_image(
        &app.source_path("allowed.png"),
        52,
        52,
        [30, 180, 90],
        [40, 40, 40],
    );
    write_pattern_image(
        &app.source_path("ignored.jpg"),
        52,
        52,
        [180, 30, 90],
        [40, 40, 40],
    );

    let mut config = app.get_json("/api/source-config").await;
    config["indexing"]["image_extensions"] = json!([".png"]);
    let updated = app
        .put_json(
            "/api/source-config",
            json!({ "indexing": config["indexing"].clone() }),
        )
        .await;
    assert_eq!(updated["indexing"]["image_extensions"], json!([".png"]));

    let indexed = app.index().await;
    assert_eq!(indexed.indexed, 1);
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);
    assert!(indexed.errors.is_empty(), "{:?}", indexed.errors);

    let payloads = app.stored_media_payloads();
    assert_eq!(payloads.len(), 1);
    assert_eq!(payloads[0].filename, "allowed.png");
}

async fn indexing_app() -> TestApp {
    TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".jpg,.png").unwrap();
        settings.duplicate_hash_distance = 0;
        settings.visual_embedding_backend = "legacy".to_string();
        settings.visual_embedding_vector_size = 32;
        settings.face_analysis_enabled = false;
        settings.ocr_enabled = false;
    })
    .await
}

fn single_payload(app: &TestApp) -> image_similarity_service::domain::models::ImagePayload {
    let payloads = app.stored_media_payloads();
    assert_eq!(payloads.len(), 1);
    payloads.into_iter().next().unwrap()
}
