use std::fs;
use std::path::Path;

use image::{Rgb, RgbImage};
use serde_json::json;

use image_similarity_service::config::parse_extensions;

mod support;

use support::harness::TestApp;
use support::media_fixtures::{
    inject_xmp_metadata, test_photo_xmp, write_pattern_image, write_test_pdf,
};

#[tokio::test]
async fn search_api_applies_metadata_filter_combinations() {
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".jpg,.png").unwrap();
        settings.default_search_limit = 10;
        settings.duplicate_hash_distance = 0;
        settings.face_analysis_enabled = false;
        settings.ocr_enabled = false;
    })
    .await;

    let matching = app.source_path("filtered-landscape.jpg");
    let wrong_orientation = app.source_path("filtered-portrait.jpg");
    let wrong_metadata = app.source_path("plain-landscape.png");
    write_pattern_image(&matching, 80, 40, [220, 30, 30], [30, 30, 30]);
    write_pattern_image(&wrong_orientation, 40, 80, [30, 80, 220], [30, 30, 30]);
    write_pattern_image(&wrong_metadata, 80, 40, [30, 180, 90], [30, 30, 30]);
    inject_xmp_metadata(&matching, test_photo_xmp());
    inject_xmp_metadata(&wrong_orientation, test_photo_xmp());

    let indexed = app.index().await;
    assert_eq!(indexed.indexed, 3);
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);

    let matching_payload = app
        .stored_media_payloads()
        .into_iter()
        .find(|payload| payload.filename == "filtered-landscape.jpg")
        .expect("filtered-landscape.jpg should be indexed");
    let tagged = app
        .put_json(
            &format!("/api/indexed-media/{}/tags", matching_payload.id),
            json!({ "tags": ["Featured"] }),
        )
        .await;
    assert_eq!(tagged["tags"], json!(["Featured"]));

    let filtered = app
        .search_upload_with_params(
            "query.jpg",
            "image/jpeg",
            fs::read(&matching).unwrap(),
            vec![
                ("limit", "10".to_string()),
                ("source_type", "local".to_string()),
                ("media_kind", "static_image".to_string()),
                ("name_query", "filtered-landscape".to_string()),
                ("camera_query", "Pocket".to_string()),
                ("keyword_query", "featured".to_string()),
                ("orientation", "landscape".to_string()),
                ("min_width", matching_payload.width.to_string()),
                ("max_width", matching_payload.width.to_string()),
                ("min_height", matching_payload.height.to_string()),
                ("max_height", matching_payload.height.to_string()),
                ("min_size_bytes", matching_payload.size_bytes.to_string()),
                ("max_size_bytes", matching_payload.size_bytes.to_string()),
                ("modified_from", matching_payload.modified_at.to_string()),
                ("modified_to", matching_payload.modified_at.to_string()),
                ("captured_from", "1710239400".to_string()),
                ("captured_to", "1710239400".to_string()),
            ],
        )
        .await;

    assert_eq!(filtered.count, 1);
    assert_eq!(filtered.results[0].image.filename, "filtered-landscape.jpg");
    assert_eq!(filtered.results[0].image.tags, vec!["Featured"]);

    let conflicting = app
        .search_upload_with_params(
            "query.jpg",
            "image/jpeg",
            fs::read(&matching).unwrap(),
            vec![
                ("limit", "10".to_string()),
                ("camera_query", "Pocket".to_string()),
                ("orientation", "portrait".to_string()),
                ("keyword_query", "featured".to_string()),
            ],
        )
        .await;
    assert_eq!(conflicting.count, 0);
}

#[tokio::test]
async fn text_only_search_matches_indexed_pdf_text_without_upload_media() {
    let app = TestApp::new(|settings| {
        settings.default_search_limit = 10;
        settings.face_analysis_enabled = false;
        settings.ocr_enabled = false;
    })
    .await;

    let pdf = app.source_path("meeting-notes.pdf");
    write_test_pdf(
        &pdf,
        &["Quarterly roadmap approval", "Receipt archive complete"],
    );

    let indexed = app.index().await;
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);
    assert!(indexed.indexed >= 2);

    let response = app
        .search_text_with_params(vec![
            ("ocr_text", "roadmap".to_string()),
            ("media_kind", "pdf_page".to_string()),
            ("limit", "10".to_string()),
        ])
        .await;

    assert_eq!(response.query_media_kind, "text");
    assert_eq!(response.query_phash, "");
    assert_eq!(response.query_ocr_text, "roadmap");
    assert_eq!(response.count, 1);
    assert_eq!(
        response.results[0].image.filename,
        "meeting-notes.pdf page 001"
    );
    assert_eq!(response.results[0].hash_distance, None);
    assert_eq!(response.results[0].ocr_score, Some(1.0));
}

#[tokio::test]
async fn search_upload_rejects_unsupported_media_with_useful_error() {
    let app = TestApp::new(|_| {}).await;

    let response = app
        .raw_search_upload(
            "notes.txt",
            "text/plain",
            b"not supported media".to_vec(),
            None,
        )
        .await;

    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(
        body["detail"],
        "Upload must be an image, video, audio, or PDF file"
    );
}

#[tokio::test]
async fn search_upload_rejects_oversize_media_at_api_boundary() {
    let app = TestApp::new(|settings| {
        settings.max_upload_mb = 1;
    })
    .await;

    let response = app
        .raw_search_upload("too-large.png", "image/png", vec![0; 1024 * 1024 + 1], None)
        .await;

    assert_eq!(response.status(), reqwest::StatusCode::PAYLOAD_TOO_LARGE);
    let body: serde_json::Value = response.json().await.unwrap();
    assert_eq!(body["detail"], "Upload is larger than 1 MB");
}

#[tokio::test]
async fn near_duplicate_filter_includes_or_excludes_by_phash_distance() {
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png").unwrap();
        settings.default_search_limit = 10;
        settings.duplicate_hash_distance = 0;
        settings.face_analysis_enabled = false;
        settings.ocr_enabled = false;
    })
    .await;

    let duplicate = app.source_path("duplicate.png");
    let different = app.source_path("different.png");
    write_diagonal_image(&duplicate, 64, 64, false);
    write_diagonal_image(&different, 64, 64, true);

    let indexed = app.index().await;
    assert_eq!(indexed.indexed, 2);
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);

    let only = app
        .search_upload_with_params(
            "duplicate-query.png",
            "image/png",
            fs::read(&duplicate).unwrap(),
            vec![
                ("limit", "10".to_string()),
                ("near_duplicate", "only".to_string()),
            ],
        )
        .await;
    assert_eq!(only.count, 1);
    assert_eq!(only.results[0].image.filename, "duplicate.png");
    assert_eq!(only.results[0].hash_distance, Some(0));
    assert!(only.results[0].near_duplicate);

    let excluded = app
        .search_upload_with_params(
            "duplicate-query.png",
            "image/png",
            fs::read(&duplicate).unwrap(),
            vec![
                ("limit", "10".to_string()),
                ("near_duplicate", "exclude".to_string()),
            ],
        )
        .await;
    assert_eq!(excluded.count, 1);
    assert_eq!(excluded.results[0].image.filename, "different.png");
    assert!(excluded.results[0].hash_distance.unwrap() > 0);
    assert!(!excluded.results[0].near_duplicate);
}

fn write_diagonal_image(path: &Path, width: u32, height: u32, inverted: bool) {
    let mut image = RgbImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let bright = if inverted { x + y > width } else { x > y };
            let value = if bright { 240 } else { 20 };
            image.put_pixel(x, y, Rgb([value, value, value]));
        }
    }
    image.save(path).unwrap();
}
