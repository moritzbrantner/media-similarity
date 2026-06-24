use std::fs;

use image_similarity_service::config::parse_extensions;

mod support;

use support::harness::TestApp;
use support::media_fixtures::write_pattern_image;

#[tokio::test]
async fn static_image_can_be_indexed_and_found_through_api_harness() {
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".jpg,.jpeg,.png").unwrap();
        settings.default_search_limit = 5;
        settings.duplicate_hash_distance = 0;
        settings.visual_embedding_backend = "legacy".to_string();
        settings.visual_embedding_vector_size = 32;
        settings.face_analysis_enabled = false;
        settings.ocr_enabled = false;
    })
    .await;

    let album = app.source_path("album");
    fs::create_dir_all(&album).unwrap();
    let red = app.source_path("album/red-landscape.jpg");
    let green = app.source_path("album/green-square.png");
    let blue = app.source_path("album/blue-portrait.jpeg");
    write_pattern_image(&red, 72, 44, [220, 30, 30], [35, 35, 35]);
    write_pattern_image(&green, 56, 56, [30, 180, 90], [40, 25, 25]);
    write_pattern_image(&blue, 44, 72, [35, 80, 220], [25, 35, 45]);

    let indexed = app.index().await;
    assert_eq!(indexed.indexed, 3);
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);
    assert!(indexed.errors.is_empty(), "{:?}", indexed.errors);

    let payloads = app.stored_media_payloads();
    assert_eq!(
        payloads
            .iter()
            .map(|payload| payload.filename.as_str())
            .collect::<Vec<_>>(),
        vec![
            "blue-portrait.jpeg",
            "green-square.png",
            "red-landscape.jpg"
        ]
    );
    for payload in &payloads {
        assert_eq!(payload.media_kind, "static_image");
        assert_eq!(payload.source_type, "local");
        assert!(payload.source_item_uri.is_some());
        assert!(payload.indexing_profile.is_some());
        assert!(payload.visual_embedding_model.is_some());
        assert!(payload.thumbnail_url.is_some());
        assert!(payload.width > 0);
        assert!(payload.height > 0);
        assert_eq!(payload.phash.len(), 16);
    }

    let response = app
        .search_upload(
            "green-query.png",
            "application/octet-stream",
            fs::read(&green).unwrap(),
            None,
        )
        .await;
    assert!(response.count >= 1);
    assert_eq!(response.results[0].image.filename, "green-square.png");
    assert_eq!(response.results[0].hash_distance, Some(0));
    assert!(response.results[0].near_duplicate);
    assert!(response
        .results
        .iter()
        .all(|result| result.image.media_kind == "static_image"));
}
