use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use jobs_core::JobSpec;
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use image_similarity_service::config::parse_extensions;
use image_similarity_service::domain::models::{
    AudioAnalysis, AudioRecognizedVoice, AudioSegmentGuess, PhotoGpsPayload, PhotoMetadataPayload,
};
use image_similarity_service::workers::media::voice::VoiceRegistry;

mod support;

use support::harness::TestApp;
use support::media_fixtures::*;

#[tokio::test]
async fn static_image_recognition_covers_content_type_extension_limits_and_duplicates() {
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png,.jpg").unwrap();
        settings.default_search_limit = 2;
        settings.duplicate_hash_distance = 0;
    })
    .await;

    let red = app.source_path("red-landscape.png");
    let green = app.source_path("green-square.png");
    let blue = app.source_path("blue-portrait.jpg");
    write_pattern_image(&red, 64, 40, [220, 20, 20], [20, 20, 20]);
    write_pattern_image(&green, 48, 48, [20, 180, 80], [20, 20, 20]);
    write_pattern_image(&blue, 40, 64, [30, 70, 220], [20, 20, 20]);
    inject_xmp_metadata(&blue, test_photo_xmp());

    let indexed = app.index().await;
    assert_eq!(indexed.indexed, 3);
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);

    let red_bytes = fs::read(&red).unwrap();
    let default_limit = app
        .search_upload("query.bin", "image/png", red_bytes.clone(), None)
        .await;
    assert_eq!(default_limit.query_media_kind, "static_image");
    assert_eq!(default_limit.count, 2);
    assert_eq!(default_limit.results[0].image.filename, "red-landscape.png");
    assert_eq!(default_limit.results[0].hash_distance, Some(0));
    assert!(default_limit.results[0].near_duplicate);
    assert!(default_limit.results.iter().all(|result| {
        result.image.media_kind == "static_image" && result.query_scene_index.is_none()
    }));

    let explicit_limit = app
        .search_upload("query.bin", "image/png", red_bytes, Some(1))
        .await;
    assert_eq!(explicit_limit.count, 1);

    let green_bytes = fs::read(&green).unwrap();
    let extension_detected = app
        .search_upload(
            "green-square.png",
            "application/octet-stream",
            green_bytes,
            Some(1),
        )
        .await;
    assert_eq!(extension_detected.count, 1);
    assert_eq!(
        extension_detected.results[0].image.filename,
        "green-square.png"
    );

    let blue_bytes = fs::read(&blue).unwrap();
    let photo_metadata = app
        .search_upload("blue-portrait.jpg", "image/jpeg", blue_bytes, Some(1))
        .await
        .results[0]
        .image
        .photo_metadata
        .clone()
        .expect("photo metadata should be indexed");
    assert_eq!(
        photo_metadata.capture_time.as_deref(),
        Some("2024-03-12T10:30:00Z")
    );
    assert_eq!(photo_metadata.camera_make.as_deref(), Some("Acme"));
    assert_eq!(photo_metadata.camera_model.as_deref(), Some("Pocket 7"));
    assert_eq!(photo_metadata.keywords, vec!["Travel", "Sunrise"]);
    assert!(photo_metadata
        .raw
        .iter()
        .any(|entry| entry.namespace == "xmp"));
}

#[tokio::test]
async fn local_static_image_reindex_replaces_stale_media_point() {
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".jpg").unwrap();
        settings.default_search_limit = 5;
        settings.duplicate_hash_distance = 0;
        settings.visual_embedding_backend = "legacy".to_string();
        settings.visual_embedding_vector_size = 32;
        settings.face_analysis_enabled = false;
        settings.ocr_enabled = false;
    })
    .await;

    let source = app.source_path("changing.jpg");
    write_pattern_image(&source, 64, 40, [220, 30, 30], [35, 35, 35]);
    let old_bytes = fs::read(&source).unwrap();

    let first = app.index().await;
    assert_eq!(first.indexed, 1);
    assert_eq!(first.failed, 0, "{:?}", first.errors);

    let first_payload = app
        .stored_media_payloads()
        .into_iter()
        .find(|payload| payload.filename == "changing.jpg")
        .expect("changing.jpg should be indexed");
    let first_modified_at = first_payload.modified_at;
    let first_size_bytes = first_payload.size_bytes;
    let first_phash = first_payload.phash.clone();
    let first_indexing_profile = first_payload.indexing_profile.clone();

    tokio::time::sleep(Duration::from_millis(20)).await;
    write_pattern_image(&source, 96, 45, [30, 180, 90], [45, 30, 25]);
    let new_bytes = fs::read(&source).unwrap();

    let second = app.index().await;
    assert_eq!(second.indexed, 1);
    assert_eq!(second.failed, 0, "{:?}", second.errors);

    let payloads = app.stored_media_payloads();
    let changing_payloads = payloads
        .iter()
        .filter(|payload| payload.filename == "changing.jpg")
        .collect::<Vec<_>>();
    assert_eq!(changing_payloads.len(), 1, "{payloads:?}");
    let current = changing_payloads[0];
    assert!(
        current.size_bytes != first_size_bytes
            || (current.modified_at - first_modified_at).abs() > 0.001
    );
    assert_eq!(current.indexing_profile, first_indexing_profile);
    assert_ne!(current.phash, first_phash);

    let new_response = app
        .search_upload(
            "changing-query.jpg",
            "application/octet-stream",
            new_bytes,
            None,
        )
        .await;
    assert!(new_response.count >= 1);
    assert_eq!(new_response.results[0].image.filename, "changing.jpg");
    assert_eq!(new_response.results[0].hash_distance, Some(0));

    let old_response = app
        .search_upload(
            "old-changing-query.jpg",
            "application/octet-stream",
            old_bytes,
            None,
        )
        .await;
    assert!(
        old_response
            .results
            .iter()
            .filter(|result| result.image.filename == "changing.jpg")
            .count()
            <= 1
    );
}

#[tokio::test]
async fn search_api_applies_server_side_metadata_filters() {
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png,.jpg").unwrap();
        settings.default_search_limit = 10;
        settings.duplicate_hash_distance = 0;
    })
    .await;

    let landscape = app.source_path("landscape.png");
    let portrait = app.source_path("blue-portrait.jpg");
    write_pattern_image(&landscape, 80, 40, [220, 20, 20], [20, 20, 20]);
    write_pattern_image(&portrait, 40, 80, [30, 70, 220], [20, 20, 20]);
    inject_xmp_metadata(&portrait, test_photo_xmp());

    let indexed = app.index().await;
    assert_eq!(indexed.indexed, 2);
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);

    let filtered = app
        .search_upload_with_params(
            "query.jpg",
            "image/jpeg",
            fs::read(&portrait).unwrap(),
            vec![
                ("limit", "10".to_string()),
                ("orientation", "portrait".to_string()),
                ("camera_query", "Pocket".to_string()),
                ("keyword_query", "sunrise".to_string()),
                ("min_width", "30".to_string()),
                ("max_width", "60".to_string()),
                ("captured_from", "1710230000".to_string()),
                ("captured_to", "1710240000".to_string()),
            ],
        )
        .await;

    assert_eq!(filtered.count, 1);
    assert_eq!(filtered.results[0].image.filename, "blue-portrait.jpg");

    let duplicate_excluded = app
        .search_upload_with_params(
            "query.jpg",
            "image/jpeg",
            fs::read(&portrait).unwrap(),
            vec![
                ("limit", "10".to_string()),
                ("near_duplicate", "exclude".to_string()),
            ],
        )
        .await;
    assert!(duplicate_excluded
        .results
        .iter()
        .all(|result| !result.near_duplicate));
}

#[tokio::test]
async fn metadata_filter_combinations_are_applied_together() {
    let app = TestApp::new(|settings| {
        settings.default_search_limit = 10;
        settings.duplicate_hash_distance = 0;
    })
    .await;
    app.state.store.ensure_collection().await.unwrap();

    let query = app.source_path("query.png");
    write_pattern_image(&query, 80, 40, [220, 20, 20], [20, 20, 20]);
    let query_media =
        image_similarity_service::workers::media::image_io::load_media(&query, &app.state.settings)
            .unwrap();
    let query_phash =
        image_similarity_service::workers::media::hashing::phash_image(&query_media.poster);

    let mut matching = test_media_payload("matching-filtered", "matching-filtered.png");
    matching.width = 80;
    matching.height = 40;
    matching.size_bytes = 2_000_000;
    matching.modified_at = 1_710_000_000.0;
    matching.phash = query_phash.clone();
    matching.media_kind = "static_image".to_string();
    matching.source_type = "s3".to_string();
    matching.source_uri = Some("s3://archive/photos".to_string());
    matching.source_item_uri = Some("s3://archive/photos/matching-filtered.png".to_string());
    matching.ocr_text = "Invoice total due".to_string();
    matching.tags = vec!["sunrise".to_string()];
    matching.people = vec![test_person_summary(
        "person-filter",
        Some("Filtered Person"),
        1,
        0.9,
    )];
    matching.photo_metadata = Some(PhotoMetadataPayload {
        capture_time: Some("2024-03-12T10:30:00Z".to_string()),
        camera_make: Some("Acme".to_string()),
        camera_model: Some("Pocket 7".to_string()),
        gps: Some(PhotoGpsPayload {
            latitude: 52.0,
            longitude: 13.0,
            altitude_meters: None,
        }),
        keywords: vec!["Travel".to_string(), "Sunrise".to_string()],
        ..PhotoMetadataPayload::default()
    });

    let mut wrong_source = matching.clone();
    wrong_source.id = "wrong-source".to_string();
    wrong_source.filename = "wrong-source.png".to_string();
    wrong_source.source_type = "local".to_string();
    wrong_source.source_uri = Some("local://archive".to_string());
    wrong_source.source_item_uri = Some("local://archive/wrong-source.png".to_string());

    let mut wrong_person = matching.clone();
    wrong_person.id = "wrong-person".to_string();
    wrong_person.filename = "wrong-person.png".to_string();
    wrong_person.people = vec![test_person_summary("other-person", None, 1, 0.7)];

    for payload in [&matching, &wrong_source, &wrong_person] {
        app.state
            .store
            .upsert_media(payload, vec![0.1; 32])
            .await
            .unwrap();
    }

    let filtered = app
        .search_upload_with_params(
            "query.png",
            "image/png",
            fs::read(&query).unwrap(),
            vec![
                ("limit", "10".to_string()),
                ("source_type", "s3".to_string()),
                ("media_kind", "static_image".to_string()),
                ("name_query", "matching".to_string()),
                ("camera_query", "Pocket".to_string()),
                ("keyword_query", "sunrise".to_string()),
                ("has_gps", "yes".to_string()),
                ("near_duplicate", "only".to_string()),
                ("orientation", "landscape".to_string()),
                ("min_width", "80".to_string()),
                ("max_width", "80".to_string()),
                ("min_height", "40".to_string()),
                ("max_height", "40".to_string()),
                ("min_size_bytes", "2000000".to_string()),
                ("max_size_bytes", "2000000".to_string()),
                ("modified_from", "1710000000".to_string()),
                ("modified_to", "1710000000".to_string()),
                ("captured_from", "1710239400".to_string()),
                ("captured_to", "1710239400".to_string()),
                ("person_id", "person-filter".to_string()),
                ("ocr_text", "invoice".to_string()),
            ],
        )
        .await;

    assert_eq!(filtered.count, 1);
    assert_eq!(filtered.results[0].image.id, "matching-filtered");
    assert_eq!(filtered.results[0].ocr_score, Some(1.0));
    assert!(filtered.results[0].near_duplicate);
}

#[tokio::test]
async fn index_reports_files_that_are_already_current_separately_from_skipped_sources() {
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png").unwrap();
    })
    .await;

    let source = app.source_path("current.png");
    write_pattern_image(&source, 64, 40, [220, 20, 20], [20, 20, 20]);

    let first = app.index().await;
    assert_eq!(first.indexed, 1);
    assert_eq!(first.skipped, 0);
    assert_eq!(first.failed, 0, "{:?}", first.errors);

    let second = app.index().await;
    assert_eq!(second.indexed, 0);
    assert_eq!(second.already_indexed, 1);
    assert_eq!(second.skipped, 0);
    assert_eq!(second.failed, 0, "{:?}", second.errors);

    write_pattern_image(&source, 65, 40, [20, 20, 220], [20, 20, 20]);
    let third = app.index().await;
    assert_eq!(third.indexed, 1);
    assert_eq!(third.failed, 0, "{:?}", third.errors);
}

#[tokio::test]
async fn index_prunes_records_for_removed_source_files() {
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png").unwrap();
    })
    .await;

    let keep = app.source_path("keep.png");
    let remove = app.source_path("remove.png");
    write_pattern_image(&keep, 64, 40, [220, 20, 20], [20, 20, 20]);
    write_pattern_image(&remove, 64, 40, [20, 20, 220], [20, 20, 20]);

    let first = app.index().await;
    assert_eq!(first.indexed, 2);
    assert_eq!(first.pruned, 0);
    assert_eq!(first.failed, 0, "{:?}", first.errors);

    fs::remove_file(&remove).unwrap();
    let second = app.index().await;
    assert_eq!(second.indexed, 0);
    assert_eq!(second.already_indexed, 1);
    assert_eq!(second.skipped, 0);
    assert_eq!(second.pruned, 1);
    assert_eq!(second.failed, 0, "{:?}", second.errors);

    let response = app
        .search_upload(
            "remove.png",
            "application/octet-stream",
            fs::read(&keep).unwrap(),
            None,
        )
        .await;
    assert_eq!(response.count, 1);
    assert_eq!(response.results[0].image.filename, "keep.png");
}

#[tokio::test]
async fn identity_person_merge_updates_inverse_index() {
    let app = TestApp::new(|settings| {
        settings.face_analysis_enabled = true;
    })
    .await;
    app.state.store.ensure_collection().await.unwrap();

    let mut payload = test_media_payload("media-people", "group.jpg");
    payload.faces = vec![
        test_face_detection("face-a", "media-people", "person-a", Some("Ada"), 0.9),
        test_face_detection("face-b", "media-people", "person-b", Some("Grace"), 0.8),
    ];
    payload.people = vec![
        test_person_summary("person-a", Some("Ada"), 1, 0.9),
        test_person_summary("person-b", Some("Grace"), 1, 0.8),
    ];
    app.state
        .store
        .upsert_media(&payload, vec![0.1; 32])
        .await
        .unwrap();
    app.state
        .store
        .upsert_face(
            &test_face_point("face-a", "media-people", "person-a", Some("Ada"), 0.9),
            vec![0.2; 32],
        )
        .await
        .unwrap();
    app.state
        .store
        .upsert_face(
            &test_face_point("face-b", "media-people", "person-b", Some("Grace"), 0.8),
            vec![0.3; 32],
        )
        .await
        .unwrap();

    let response = app
        .post_json(
            "/api/identities/people/person-a/merge",
            json!({ "source_ids": ["person-b"] }),
        )
        .await;
    assert_eq!(response["target_id"], "person-a");
    assert_eq!(response["updated_media"], 1);
    assert_eq!(response["updated_faces"], 1);

    let inverse = app.get_json("/api/inverse-index").await;
    let people = inverse["people"].as_array().unwrap();
    assert_eq!(people.len(), 1, "{people:?}");
    assert_eq!(people[0]["id"], "person-a");
    assert_eq!(people[0]["label"], "Ada");
    assert_eq!(people[0]["face_count"], 2);
    assert_eq!(people[0]["media_count"], 1);
}

#[tokio::test]
async fn identity_merge_handles_missing_sources_without_corrupting_target() {
    let app = TestApp::new(|settings| {
        settings.face_analysis_enabled = true;
    })
    .await;
    app.state.store.ensure_collection().await.unwrap();

    let mut payload = test_media_payload("media-people-missing", "group-missing.jpg");
    payload.faces = vec![
        test_face_detection(
            "face-a",
            "media-people-missing",
            "person-a",
            Some("Ada"),
            0.9,
        ),
        test_face_detection(
            "face-b",
            "media-people-missing",
            "person-b",
            Some("Grace"),
            0.8,
        ),
    ];
    payload.people = vec![
        test_person_summary("person-a", Some("Ada"), 1, 0.9),
        test_person_summary("person-b", Some("Grace"), 1, 0.8),
    ];
    app.state
        .store
        .upsert_media(&payload, vec![0.1; 32])
        .await
        .unwrap();
    app.state
        .store
        .upsert_face(
            &test_face_point(
                "face-b",
                "media-people-missing",
                "person-b",
                Some("Grace"),
                0.8,
            ),
            vec![0.3; 32],
        )
        .await
        .unwrap();

    let response = app
        .post_json(
            "/api/identities/people/person-a/merge",
            json!({ "source_ids": ["person-b", "person-missing"] }),
        )
        .await;
    assert_eq!(response["target_id"], "person-a");
    assert_eq!(response["updated_media"], 1);
    assert_eq!(response["updated_faces"], 1);
    assert!(response["warnings"]
        .as_array()
        .unwrap()
        .iter()
        .any(|warning| warning.as_str().unwrap().contains("person-missing")));

    let inverse = app.get_json("/api/inverse-index").await;
    let people = inverse["people"].as_array().unwrap();
    assert_eq!(people.len(), 1, "{people:?}");
    assert_eq!(people[0]["id"], "person-a");
    assert_eq!(people[0]["face_count"], 2);
}

#[tokio::test]
async fn identity_speaker_rename_updates_inverse_index_and_registry() {
    let app = TestApp::new(|_| {}).await;
    app.state.store.ensure_collection().await.unwrap();
    seed_voice_registry(&app);

    let mut payload = test_media_payload("media-audio", "interview.mp3");
    payload.media_kind = "audio".to_string();
    payload.full_audio_url = Some("/uploads/interview.mp3".to_string());
    payload.audio_analysis = Some(AudioAnalysis {
        speech_detected: true,
        speech_ratio: 1.0,
        speech_segments: Vec::new(),
        audio_segments: vec![AudioSegmentGuess {
            segment_index: 0,
            kind: "speech".to_string(),
            start_seconds: 1.0,
            end_seconds: 3.0,
            confidence: 0.75,
            speaker_id: Some("voice-0001".to_string()),
            speaker_label: Some("Voice 1".to_string()),
        }],
        recognized_voices: vec![AudioRecognizedVoice {
            id: "voice-0001".to_string(),
            label: "Voice 1".to_string(),
            segment_count: 1,
            total_seconds: 2.0,
            confidence: 0.75,
        }],
        transcript_text: String::new(),
        transcript_language: None,
        transcript_segments: Vec::new(),
        tempo_bpm: None,
        tempo_confidence: 0.0,
        tempo_onset_count: 0,
    });
    app.state
        .store
        .upsert_media(&payload, vec![0.1; 32])
        .await
        .unwrap();

    let response = app
        .put_json(
            "/api/identities/speakers/voice-0001",
            json!({ "label": "Alice" }),
        )
        .await;
    assert_eq!(response["target_label"], "Alice");
    assert_eq!(response["registry_updated"], true);

    let inverse = app.get_json("/api/inverse-index").await;
    assert_eq!(inverse["speakers"][0]["id"], "voice-0001");
    assert_eq!(inverse["speakers"][0]["label"], "Alice");

    let registry = VoiceRegistry::load(&app.state.settings).unwrap();
    assert_eq!(
        registry.label("voice-0001").unwrap().as_deref(),
        Some("Alice")
    );
}

#[tokio::test]
async fn smart_album_crud_filters_text_and_duplicate_groups() {
    let app = TestApp::new(|settings| {
        settings.duplicate_hash_distance = 1;
    })
    .await;
    app.state.store.ensure_collection().await.unwrap();

    let mut invoice = test_media_payload("invoice-page", "invoice.pdf page 001");
    invoice.media_kind = "pdf_page".to_string();
    invoice.ocr_text = "Invoice total due".to_string();
    invoice.phash = "0000000000000000".to_string();
    app.state
        .store
        .upsert_media(&invoice, vec![0.1; 32])
        .await
        .unwrap();

    let mut duplicate = test_media_payload("invoice-copy", "invoice-copy.jpg");
    duplicate.ocr_text = "Invoice copy".to_string();
    duplicate.phash = "0000000000000001".to_string();
    app.state
        .store
        .upsert_media(&duplicate, vec![0.1; 32])
        .await
        .unwrap();

    let mut other = test_media_payload("receipt-page", "receipt.pdf page 001");
    other.media_kind = "pdf_page".to_string();
    other.ocr_text = "Receipt archive".to_string();
    other.phash = "ffffffffffffffff".to_string();
    app.state
        .store
        .upsert_media(&other, vec![0.1; 32])
        .await
        .unwrap();

    let created = app
        .post_json(
            "/api/smart-albums",
            json!({
                "name": "Invoices",
                "description": null,
                "criteria": {
                    "text_query": "invoice",
                    "duplicate_status": "only"
                },
                "sort": "duplicate_group_size",
                "limit": 20
            }),
        )
        .await;
    let album_id = created["id"].as_str().unwrap();

    let results = app
        .get_json(&format!("/api/smart-albums/{album_id}/results"))
        .await;
    assert_eq!(results["total"], 2);
    assert_eq!(results["duplicate_groups"].as_array().unwrap().len(), 1);
    assert!(results["results"]
        .as_array()
        .unwrap()
        .iter()
        .all(|result| result["duplicate_group_size"] == 2));

    let updated = app
        .put_json(
            &format!("/api/smart-albums/{album_id}"),
            json!({
                "name": "PDF invoices",
                "description": null,
                "criteria": {
                    "media_kind": "pdf_page",
                    "text_query": "invoice",
                    "duplicate_status": "all"
                },
                "sort": "modified_newest",
                "limit": 20
            }),
        )
        .await;
    assert_eq!(updated["name"], "PDF invoices");
    let filtered = app
        .get_json(&format!("/api/smart-albums/{album_id}/results"))
        .await;
    assert_eq!(filtered["total"], 1);
    assert_eq!(filtered["results"][0]["image"]["id"], "invoice-page");

    let deleted = app
        .delete_json(&format!("/api/smart-albums/{album_id}"))
        .await;
    assert_eq!(deleted["deleted"], true);
    assert!(app.get_json("/api/smart-albums").await["albums"]
        .as_array()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn smart_album_pagination_sorting_and_invalid_inputs() {
    let app = TestApp::new(|settings| {
        settings.duplicate_hash_distance = 1;
    })
    .await;
    app.state.store.ensure_collection().await.unwrap();

    for index in 0..6 {
        let mut payload =
            test_media_payload(&format!("album-{index}"), &format!("item-{index}.jpg"));
        payload.modified_at = 1_700_000_000.0 + index as f64;
        payload.phash = if index < 3 {
            format!("000000000000000{index}")
        } else {
            "ffffffffffffffff".to_string()
        };
        payload.ocr_text = if index % 2 == 0 {
            "Invoice archive".to_string()
        } else {
            "Receipt archive".to_string()
        };
        app.state
            .store
            .upsert_media(&payload, vec![0.1; 32])
            .await
            .unwrap();
    }

    let created = app
        .post_json(
            "/api/smart-albums",
            json!({
                "name": "Paged",
                "description": null,
                "criteria": { "text_query": "archive", "duplicate_status": "all" },
                "sort": "modified_newest",
                "limit": 2
            }),
        )
        .await;
    let album_id = created["id"].as_str().unwrap();

    let first_page = app
        .get_json(&format!(
            "/api/smart-albums/{album_id}/results?offset=0&limit=2"
        ))
        .await;
    assert_eq!(first_page["count"], 2);
    assert_eq!(first_page["total"], 6);
    assert_eq!(first_page["results"][0]["image"]["filename"], "item-5.jpg");
    assert_eq!(first_page["results"][1]["image"]["filename"], "item-4.jpg");

    let second_page = app
        .get_json(&format!(
            "/api/smart-albums/{album_id}/results?offset=2&limit=2"
        ))
        .await;
    assert_eq!(second_page["offset"], 2);
    assert_eq!(second_page["results"][0]["image"]["filename"], "item-3.jpg");

    let duplicate_preview = app
        .post_json(
            "/api/smart-albums/preview?offset=0&limit=10",
            json!({
                "name": "Duplicates",
                "description": null,
                "criteria": { "duplicate_status": "only" },
                "sort": "duplicate_group_size",
                "limit": 10
            }),
        )
        .await;
    assert_eq!(
        duplicate_preview["duplicate_groups"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(duplicate_preview["results"]
        .as_array()
        .unwrap()
        .iter()
        .all(|result| result["duplicate_group_size"].as_u64().unwrap() >= 3));

    let invalid = app
        .raw_post_json(
            "/api/smart-albums",
            json!({
                "name": "Invalid",
                "description": null,
                "criteria": { "media_kind": "spreadsheet" },
                "sort": "modified_newest",
                "limit": 0
            }),
        )
        .await;
    assert_eq!(invalid.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn deletes_indexed_media_records_and_generated_artifacts() {
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png").unwrap();
    })
    .await;

    let source = app.source_path("delete-me.png");
    write_pattern_image(&source, 64, 40, [220, 20, 20], [20, 20, 20]);
    let indexed = app.index().await;
    assert_eq!(indexed.indexed, 1);

    let before = app
        .search_upload(
            "query.png",
            "application/octet-stream",
            fs::read(&source).unwrap(),
            None,
        )
        .await;
    assert_eq!(before.count, 1);
    let media_id = before.results[0].image.id.clone();

    let deleted: Value = app
        .client
        .delete(format!("{}/api/indexed-media/{media_id}", app.base_url))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(deleted["deleted_points"], 1);
    assert!(deleted["deleted_artifacts"].as_u64().unwrap_or_default() >= 1);

    let after = app
        .search_upload(
            "query.png",
            "application/octet-stream",
            fs::read(&source).unwrap(),
            None,
        )
        .await;
    assert_eq!(after.count, 0);
}

#[tokio::test]
async fn delete_indexed_source_removes_only_matching_source_records() {
    let app = TestApp::new(|settings| {
        let root = settings.source_image_dir.parent().unwrap().to_path_buf();
        let source_a = root.join("source-a");
        let source_b = root.join("source-b");
        fs::create_dir_all(&source_a).unwrap();
        fs::create_dir_all(&source_b).unwrap();
        settings.image_sources = vec![
            source_a.to_string_lossy().to_string(),
            source_b.to_string_lossy().to_string(),
        ];
        settings.image_extensions = parse_extensions(".png").unwrap();
        settings.default_search_limit = 10;
    })
    .await;
    let source_a = app.root_path().join("source-a");
    let source_b = app.root_path().join("source-b");
    let delete_by_source = source_a.join("delete-source.png");
    let keep_after_source_delete = source_b.join("keep-source.png");
    write_pattern_image(&delete_by_source, 64, 40, [220, 20, 20], [20, 20, 20]);
    write_pattern_image(
        &keep_after_source_delete,
        64,
        40,
        [20, 180, 80],
        [20, 20, 20],
    );

    let indexed = app.index().await;
    assert_eq!(indexed.indexed, 2);
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);

    let deleted_by_source = app
        .delete_json(&format!(
            "/api/indexed-sources?source_uri={}",
            source_a.to_string_lossy()
        ))
        .await;
    assert_eq!(deleted_by_source["deleted_points"], 1);

    let after_source_delete = app
        .search_upload(
            "query.png",
            "image/png",
            fs::read(&keep_after_source_delete).unwrap(),
            Some(10),
        )
        .await;
    assert_eq!(after_source_delete.count, 1);
    assert_eq!(
        after_source_delete.results[0].image.filename,
        "keep-source.png"
    );

    let keep_payload = app
        .stored_media_payloads()
        .into_iter()
        .find(|payload| payload.filename == "keep-source.png")
        .unwrap();
    let deleted_by_item = app
        .delete_json(&format!(
            "/api/indexed-sources?source_item_uri={}",
            keep_payload.source_item_uri.unwrap()
        ))
        .await;
    assert_eq!(deleted_by_item["deleted_points"], 1);

    let after_item_delete = app
        .search_upload(
            "query.png",
            "image/png",
            fs::read(&keep_after_source_delete).unwrap(),
            Some(10),
        )
        .await;
    assert_eq!(after_item_delete.count, 0);

    let missing_filter = app
        .client
        .delete(format!("{}/api/indexed-sources", app.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(missing_filter.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn source_extension_configuration_filters_indexed_media() {
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png").unwrap();
        settings.default_search_limit = 10;
    })
    .await;

    let included = app.source_path("included.PNG");
    let excluded = app.source_path("excluded.jpg");
    write_pattern_image(&included, 40, 40, [200, 40, 40], [10, 10, 10]);
    write_pattern_image(&excluded, 40, 40, [40, 40, 200], [10, 10, 10]);

    let indexed = app.index().await;
    assert_eq!(indexed.indexed, 1);
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);

    let response = app
        .search_upload(
            "included.PNG",
            "application/octet-stream",
            fs::read(&included).unwrap(),
            None,
        )
        .await;
    assert_eq!(response.count, 1);
    assert_eq!(response.results[0].image.filename, "included.PNG");
}

#[tokio::test]
async fn gif_recognition_indexes_animation_metadata_and_honors_gif_configuration() {
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".gif").unwrap();
        settings.default_search_limit = 5;
        settings.gif_sample_frames = 3;
        settings.gif_preview_frames = 2;
        settings.gif_max_decode_frames = 4;
        settings.gif_motion_weight = 0.75;
    })
    .await;

    let gif = app.source_path("motion.gif");
    write_test_gif(
        &gif,
        &[
            [220, 40, 40],
            [40, 220, 40],
            [40, 40, 220],
            [220, 220, 40],
            [220, 40, 220],
        ],
        70,
    );

    let indexed = app.index().await;
    assert_eq!(indexed.indexed, 1);
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);

    let response = app
        .search_upload("motion.gif", "image/gif", fs::read(&gif).unwrap(), None)
        .await;
    assert_eq!(response.query_media_kind, "animated_gif");
    assert_eq!(response.count, 1);

    let image = &response.results[0].image;
    assert_eq!(image.filename, "motion.gif");
    assert_eq!(image.media_kind, "animated_gif");
    assert_eq!(image.frame_count, Some(4));
    assert_eq!(image.duration_ms, Some(280));
    assert!(image.animated_thumbnail_url.is_some());
}

#[tokio::test]
async fn video_recognition_indexes_source_scenes_and_searches_uploaded_scenes() {
    if !has_tool("ffmpeg") || !has_tool("ffprobe") {
        eprintln!("skipping video e2e test because ffmpeg/ffprobe is unavailable");
        return;
    }

    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".mp4").unwrap();
        settings.default_search_limit = 5;
        settings.video_frame_stride = 3;
        settings.video_max_frames = Some(4);
        settings.gif_motion_weight = 0.4;
    })
    .await;

    let video = app.source_path("two-scenes.mp4");
    write_two_scene_video(&video);

    let indexed = app.index().await;
    assert!(indexed.indexed >= 1, "{indexed:?}");
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);

    let response = app
        .search_upload("query.mp4", "video/mp4", fs::read(&video).unwrap(), Some(3))
        .await;
    assert_eq!(response.query_media_kind, "video");
    assert!(!response.scenes.is_empty());
    assert_eq!(response.count, response.results.len());
    assert!(response.results.iter().all(|result| {
        result.image.media_kind == "video_scene" && result.query_scene_index.is_some()
    }));

    let scene = &response.scenes[0];
    assert_eq!(scene.scene_kind, "scene");
    assert_eq!(scene.clip_url.as_deref(), None);
    assert!(scene.end_seconds > scene.start_seconds);
    assert!(scene.count <= 3);
    assert!(scene.results.iter().all(|result| {
        result.image.full_video_url.is_some() && result.image.scene_clip_url.is_some()
    }));
    let top_level_upload_entries = fs::read_dir(&app.state.settings.upload_dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect::<Vec<_>>();
    assert!(
        top_level_upload_entries
            .iter()
            .all(|name| !name.starts_with("query-")),
        "{top_level_upload_entries:?}"
    );
}

#[tokio::test]
async fn audio_recognition_indexes_bits_voice_metadata_and_searches_uploaded_audio() {
    if !has_tool("ffmpeg") || !has_tool("ffprobe") {
        eprintln!("skipping audio e2e test because ffmpeg/ffprobe is unavailable");
        return;
    }

    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".wav").unwrap();
        settings.audio_extensions = parse_extensions(".wav").unwrap();
        settings.default_search_limit = 4;
    })
    .await;

    let audio = app.source_path("voice-like-tone.wav");
    write_voice_like_audio(&audio);

    let indexed = app.index().await;
    assert!(indexed.indexed >= 1, "{indexed:?}");
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);

    let response = app
        .search_upload("query.wav", "audio/wav", fs::read(&audio).unwrap(), Some(2))
        .await;
    assert_eq!(response.query_media_kind, "audio");
    assert!(!response.scenes.is_empty());
    assert!(response.query_audio_analysis.is_some());
    assert_eq!(response.count, response.results.len());
    assert!(response.results.iter().all(|result| {
        result.image.media_kind == "audio" && result.query_scene_index.is_some()
    }));

    let first_scene = &response.scenes[0];
    assert_eq!(first_scene.scene_kind, "audio_bit");
    assert!(first_scene.end_seconds > first_scene.start_seconds);
    assert!(first_scene.count <= 2);

    let result_audio = response.results[0].image.audio_analysis.as_ref().unwrap();
    assert!(result_audio.speech_detected);
    assert!(!result_audio.audio_segments.is_empty());
    assert!(!result_audio.recognized_voices.is_empty());
    assert!(response.results[0].image.full_audio_url.is_some());
}

#[tokio::test]
async fn pdf_recognition_indexes_document_pages_and_searches_uploaded_pdf() {
    if !has_tool("pdfinfo") || !has_tool("pdftoppm") || !has_tool("pdftotext") {
        eprintln!("skipping PDF e2e test because poppler-utils is unavailable");
        return;
    }

    let app = TestApp::new(|settings| {
        settings.default_search_limit = 10;
        settings.ocr_enabled = false;
        settings.pdf_max_pages = 10;
        settings.pdf_summary_pages = 2;
    })
    .await;

    let pdf = app.source_path("invoice.pdf");
    write_test_pdf(&pdf, &["Invoice total due", "Receipt archive"]);

    let indexed = app.index().await;
    assert_eq!(indexed.indexed, 3);
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);

    let second = app.index().await;
    assert_eq!(second.indexed, 0);
    assert_eq!(second.already_indexed, 1);
    assert_eq!(second.skipped, 0);

    let response = app
        .search_upload(
            "query.pdf",
            "application/pdf",
            fs::read(&pdf).unwrap(),
            Some(10),
        )
        .await;
    assert_eq!(response.query_media_kind, "pdf");
    assert_eq!(response.scenes.len(), 2);
    assert!(response
        .scenes
        .iter()
        .all(|scene| scene.scene_kind == "pdf_page"));
    assert!(response.results.iter().any(|result| {
        result.image.media_kind == "pdf_document"
            && result.image.full_pdf_url.is_some()
            && result.image.pdf_page_count == Some(2)
    }));
    assert!(response.results.iter().any(|result| {
        result.image.media_kind == "pdf_page"
            && result.image.pdf_page_number == Some(1)
            && result.image.pdf_page_count == Some(2)
            && result.image.pdf_page_url.is_some()
            && result.image.ocr_text.contains("Invoice")
    }));
}

#[tokio::test]
async fn mixed_media_indexing_reports_expected_record_shapes() {
    let ffmpeg_available = has_tool("ffmpeg") && has_tool("ffprobe");
    let poppler_available = has_tool("pdfinfo") && has_tool("pdftoppm") && has_tool("pdftotext");
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png,.gif,.wav,.mp4,.pdf").unwrap();
        settings.audio_extensions = parse_extensions(".wav").unwrap();
        settings.pdf_extensions = parse_extensions(".pdf").unwrap();
        settings.default_search_limit = 10;
        settings.video_frame_stride = 3;
        settings.video_max_frames = Some(4);
        settings.pdf_max_pages = 4;
        settings.pdf_summary_pages = 2;
        settings.ocr_enabled = false;
    })
    .await;

    write_pattern_image(
        &app.source_path("still.png"),
        48,
        32,
        [220, 30, 30],
        [20, 20, 20],
    );
    write_test_gif(
        &app.source_path("motion.gif"),
        &[[220, 40, 40], [40, 220, 40], [40, 40, 220]],
        60,
    );
    if ffmpeg_available {
        write_voice_like_audio(&app.source_path("voice.wav"));
        write_two_scene_video(&app.source_path("scene.mp4"));
    }
    if poppler_available {
        write_test_pdf(&app.source_path("doc.pdf"), &["Invoice", "Archive"]);
    }

    let indexed = app.index().await;
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);
    assert!(indexed.indexed >= 2, "{indexed:?}");

    let payloads = app.stored_media_payloads();
    assert!(payloads.iter().any(|payload| {
        payload.filename == "still.png"
            && payload.media_kind == "static_image"
            && payload.thumbnail_url.is_some()
    }));
    assert!(payloads.iter().any(|payload| {
        payload.filename == "motion.gif"
            && payload.media_kind == "animated_gif"
            && payload.frame_count == Some(3)
            && payload.animated_thumbnail_url.is_some()
    }));
    if ffmpeg_available {
        assert!(payloads.iter().any(|payload| {
            payload.media_kind == "audio"
                && payload.full_audio_url.is_some()
                && payload.audio_analysis.is_some()
        }));
        assert!(payloads.iter().any(|payload| {
            payload.media_kind == "video_scene"
                && payload.full_video_url.is_some()
                && payload.scene_clip_url.is_some()
                && payload.scene_end_seconds > payload.scene_start_seconds
        }));
    }
    if poppler_available {
        assert!(payloads.iter().any(|payload| {
            payload.media_kind == "pdf_document"
                && payload.full_pdf_url.is_some()
                && payload.pdf_page_count == Some(2)
        }));
        assert!(payloads.iter().any(|payload| {
            payload.media_kind == "pdf_page"
                && payload.pdf_page_url.is_some()
                && payload.pdf_page_number == Some(1)
                && payload.pdf_page_count == Some(2)
        }));
    }
}

#[tokio::test]
async fn upload_validation_covers_invalid_media_and_size_configuration() {
    let app = TestApp::new(|settings| {
        settings.max_upload_mb = 1;
    })
    .await;

    let invalid = app
        .raw_search_upload("notes.txt", "text/plain", b"not media".to_vec(), None)
        .await;
    assert_eq!(invalid.status(), reqwest::StatusCode::BAD_REQUEST);
    let invalid_body: Value = invalid.json().await.unwrap();
    assert_eq!(
        invalid_body["detail"],
        "Upload must be an image, video, audio, or PDF file"
    );

    let malformed = app
        .client
        .post(format!("{}/api/search", app.base_url))
        .header(CONTENT_TYPE, "multipart/form-data; boundary=missing-body")
        .body("not a valid multipart body")
        .send()
        .await
        .unwrap();
    assert_eq!(malformed.status(), reqwest::StatusCode::BAD_REQUEST);

    let oversized = app
        .raw_search_upload("large.png", "image/png", vec![0_u8; 1024 * 1024 + 1], None)
        .await;
    assert_eq!(oversized.status(), reqwest::StatusCode::PAYLOAD_TOO_LARGE);

    let body_limit_exceeded = app
        .raw_search_upload(
            "huge.png",
            "image/png",
            vec![0_u8; 1024 * 1024 + 70 * 1024],
            None,
        )
        .await;
    let body_limit_status = body_limit_exceeded.status();
    let body_limit_body = body_limit_exceeded.text().await.unwrap();
    assert_eq!(
        body_limit_status,
        reqwest::StatusCode::PAYLOAD_TOO_LARGE,
        "{body_limit_body}"
    );
}

#[tokio::test]
async fn source_config_api_persists_sources_and_reports_planned_types() {
    let app = TestApp::new(|_| {}).await;
    let extra_source = app.root_path().join("extra-media");
    fs::create_dir_all(&extra_source).unwrap();

    let initial = app.get_json("/api/source-config").await;
    assert_eq!(
        initial["default_source_dir"].as_str(),
        Some(app.source_dir.to_string_lossy().as_ref())
    );
    assert_eq!(initial["sources"][0]["kind"], "local");
    assert_eq!(initial["sources"][0]["status"], "ready");
    assert_eq!(initial["media_sources_writable"], true);
    assert_eq!(
        initial["supported_source_types"]
            .as_array()
            .unwrap()
            .iter()
            .find(|source_type| source_type["kind"] == "minio")
            .unwrap()["implemented"],
        true
    );

    let updated = app
        .put_json(
            "/api/source-config",
            json!({
                "sources": [
                    format!("  {}  ", extra_source.display()),
                    "",
                    "minio://bucket/prefix",
                    "s3://archive/photos",
                    "video:///clips/demo.mp4",
                    "camera://front-door"
                ]
            }),
        )
        .await;
    assert_eq!(
        updated["sources"][0]["spec"].as_str(),
        Some(extra_source.to_string_lossy().as_ref())
    );
    assert_eq!(updated["sources"][0]["status"], "ready");
    assert_eq!(updated["sources"][1]["kind"], "minio");
    assert_eq!(updated["sources"][1]["status"], "unavailable");
    assert_eq!(updated["sources"][2]["kind"], "s3");
    assert_eq!(updated["sources"][2]["status"], "ready");
    assert_eq!(updated["sources"][3]["kind"], "video");
    assert_eq!(updated["sources"][3]["status"], "not_implemented");
    assert_eq!(updated["sources"][4]["kind"], "camera");
    assert_eq!(updated["sources"][4]["status"], "not_implemented");

    let persisted = fs::read_to_string(app.media_sources_file()).unwrap();
    assert!(persisted.contains("# Managed by image-similarity-service."));
    assert!(persisted.contains(&extra_source.to_string_lossy().to_string()));
    assert!(persisted.contains("minio://bucket/prefix"));
    assert!(persisted.contains("s3://archive/photos"));

    let reloaded = app.get_json("/api/source-config").await;
    assert_eq!(reloaded["sources"], updated["sources"]);

    let empty = app
        .raw_put_json("/api/source-config", json!({ "sources": ["  "] }))
        .await;
    assert_eq!(empty.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: Value = empty.json().await.unwrap();
    assert_eq!(
        body["detail"],
        "At least one media source must be configured"
    );
}

#[tokio::test]
async fn source_config_api_reports_seed_file_and_writes_target_file() {
    let app = TestApp::new(|settings| {
        settings.media_sources_seed_file = Some(
            settings
                .media_sources_file
                .with_file_name("seed-media-sources.txt"),
        );
    })
    .await;
    let seed_file = app
        .state
        .settings
        .media_sources_seed_file
        .as_ref()
        .unwrap()
        .clone();
    fs::create_dir_all(seed_file.parent().unwrap()).unwrap();
    fs::write(&seed_file, "/seed\n").unwrap();

    let initial = app.get_json("/api/source-config").await;
    assert_eq!(
        initial["media_sources_file"].as_str(),
        Some(app.media_sources_file().to_string_lossy().as_ref())
    );
    assert_eq!(
        initial["media_sources_seed_file"].as_str(),
        Some(seed_file.to_string_lossy().as_ref())
    );
    assert_eq!(initial["media_sources_writable"], true);

    let target_source = app.root_path().join("target-media");
    fs::create_dir_all(&target_source).unwrap();
    app.put_json(
        "/api/source-config",
        json!({ "sources": [target_source.to_string_lossy().to_string()] }),
    )
    .await;

    assert!(fs::read_to_string(app.media_sources_file())
        .unwrap()
        .contains(target_source.to_string_lossy().as_ref()));
    assert_eq!(fs::read_to_string(seed_file).unwrap(), "/seed\n");
}

#[tokio::test]
async fn source_config_api_updates_runtime_indexing_configuration() {
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png").unwrap();
    })
    .await;

    let updated = app
        .put_json(
            "/api/source-config",
            json!({
                "indexing": {
                    "image_extensions": [".png", "webp"],
                    "audio_extensions": [".mp3"],
                    "pdf_extensions": ["pdf"],
                    "face_analysis_enabled": false,
                    "visual_embedding_enabled": false,
                    "face_detection_min_confidence": 0.6,
                    "face_cluster_threshold": 0.4,
                    "face_min_cluster_images": 3,
                    "face_max_frames_per_media": 6,
                    "gif_sample_frames": 8,
                    "gif_max_decode_frames": 128,
                    "gif_preview_frames": 6,
                    "gif_default_frame_delay_ms": 90,
                    "gif_motion_weight": 0.35,
                    "video_frame_stride": 12,
                    "video_max_frames": 48,
                    "pdf_render_dpi": 180,
                    "pdf_max_pages": 12,
                    "pdf_summary_pages": 4,
                    "ocr_enabled": false,
                    "ocr_max_frames": 2,
                    "audio_transcription_enabled": true
                }
            }),
        )
        .await;

    assert_eq!(updated["indexing"]["image_extensions"][0], ".png");
    assert_eq!(updated["indexing"]["image_extensions"][1], ".webp");
    assert_eq!(updated["indexing"]["audio_extensions"][0], ".mp3");
    assert_eq!(updated["indexing"]["pdf_extensions"][0], ".pdf");
    assert_eq!(updated["indexing"]["face_analysis_enabled"], false);
    assert_eq!(updated["indexing"]["visual_embedding_enabled"], false);
    assert_eq!(updated["indexing"]["video_frame_stride"], 12);
    assert_eq!(updated["indexing"]["video_max_frames"], 48);
    assert_eq!(updated["indexing"]["ocr_enabled"], false);

    let invalid = app
        .raw_put_json(
            "/api/source-config",
            json!({
                "indexing": {
                    "image_extensions": [".png"],
                    "audio_extensions": [".mp3"],
                    "pdf_extensions": [".pdf"],
                    "face_analysis_enabled": true,
                    "face_detection_min_confidence": 1.4,
                    "face_cluster_threshold": 0.4,
                    "face_min_cluster_images": 1,
                    "face_max_frames_per_media": 1,
                    "gif_sample_frames": 1,
                    "gif_max_decode_frames": 1,
                    "gif_preview_frames": 1,
                    "gif_default_frame_delay_ms": 1,
                    "gif_motion_weight": 0.2,
                    "video_frame_stride": 1,
                    "video_max_frames": null,
                    "pdf_render_dpi": 144,
                    "pdf_max_pages": 1,
                    "pdf_summary_pages": 1,
                    "ocr_enabled": true,
                    "ocr_max_frames": 1,
                    "audio_transcription_enabled": false
                }
            }),
        )
        .await;
    assert_eq!(invalid.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn readiness_reports_ready_when_required_dependencies_are_available() {
    let app = TestApp::new(|settings| {
        settings.visual_embedding_enabled = false;
        settings.face_analysis_enabled = false;
        settings.audio_transcription_enabled = false;
        settings.ocr_enabled = false;
    })
    .await;

    let response = app.raw_get("/api/ready").await;

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], "ready");
    assert!(body["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| { check["name"] == "qdrant" && check["status"] == "ok" }));
}

#[tokio::test]
async fn readiness_creates_qdrant_payload_indexes() {
    let app = TestApp::new(|settings| {
        settings.visual_embedding_enabled = false;
        settings.face_analysis_enabled = false;
        settings.audio_transcription_enabled = false;
        settings.ocr_enabled = false;
    })
    .await;

    let response = app.raw_get("/api/ready").await;

    assert_eq!(response.status(), reqwest::StatusCode::OK);
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
                .and_then(|value| value.get("data_type"))
                .and_then(Value::as_str),
            Some(data_type),
            "missing or invalid payload index {field}"
        );
    }
}

#[tokio::test]
async fn readiness_succeeds_when_qdrant_payload_indexes_already_exist() {
    let app = TestApp::new(|settings| {
        settings.visual_embedding_enabled = false;
        settings.face_analysis_enabled = false;
        settings.audio_transcription_enabled = false;
        settings.ocr_enabled = false;
    })
    .await;

    let first = app.raw_get("/api/ready").await;
    let second = app.raw_get("/api/ready").await;

    assert_eq!(first.status(), reqwest::StatusCode::OK);
    assert_eq!(second.status(), reqwest::StatusCode::OK);
}

#[tokio::test]
async fn readiness_reports_not_ready_when_required_visual_model_is_missing() {
    let app = TestApp::new(|settings| {
        let root = settings.source_image_dir.parent().unwrap().to_path_buf();
        settings.visual_embedding_enabled = true;
        settings.visual_embedding_backend = "onnx".to_string();
        settings.visual_embedding_model_path = root.join("missing-model.onnx");
        settings.visual_embedding_preprocessor_path = root.join("missing-preprocessor.json");
        settings.model_bundle_dir = root.join("missing-bundles");
        settings.face_analysis_enabled = false;
        settings.audio_transcription_enabled = false;
        settings.ocr_enabled = false;
    })
    .await;

    let response = app.raw_get("/api/ready").await;

    assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], "not_ready");
    let visual = body["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == "model.visual_embedding")
        .unwrap();
    assert_eq!(visual["status"], "error");
    assert!(visual["detail"]
        .as_str()
        .unwrap()
        .contains("/api/models/visual_embedding/download"));
}

#[tokio::test]
async fn readiness_fails_when_qdrant_payload_index_creation_fails() {
    let app = TestApp::new(|settings| {
        settings.qdrant_collection = format!("fail-payload-index-{}", Uuid::new_v4());
        settings.qdrant_retry_attempts = 0;
        settings.ocr_enabled = false;
    })
    .await;

    let response = app.raw_get("/api/ready").await;

    assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
    let body: Value = response.json().await.unwrap();
    let qdrant = body["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == "qdrant")
        .unwrap();
    assert_eq!(qdrant["status"], "error");
    let detail = qdrant["detail"].as_str().unwrap();
    assert!(detail.contains("Qdrant create_payload_index failed"));
    assert!(detail.contains("fail-payload-index"));
    assert!(detail.contains("HTTP 503"));
}

#[tokio::test]
async fn readiness_reports_not_ready_when_qdrant_is_unavailable() {
    let app = TestApp::new(|settings| {
        settings.qdrant_url = "http://127.0.0.1:9".to_string();
        settings.qdrant_request_timeout_ms = 1_000;
        settings.qdrant_connect_timeout_ms = 100;
        settings.qdrant_retry_attempts = 0;
        settings.ocr_enabled = false;
    })
    .await;

    let start = Instant::now();
    let response = app.raw_get("/api/ready").await;

    assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
    assert!(start.elapsed() < Duration::from_secs(2));
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], "not_ready");
    assert!(body["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| { check["name"] == "qdrant" && check["status"] == "error" }));
}

#[tokio::test]
async fn readiness_keeps_optional_tool_failures_as_warnings() {
    let app = TestApp::new(|settings| {
        settings.visual_embedding_enabled = false;
        settings.face_analysis_enabled = false;
        settings.audio_transcription_enabled = false;
        settings.ocr_enabled = true;
        settings.ocr_command = "definitely-missing-image-sim-ocr".to_string();
    })
    .await;

    let response = app.raw_get("/api/ready").await;

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], "ready");
    assert!(body["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| { check["name"] == "ocr" && check["status"] == "warn" }));
}

#[tokio::test]
async fn health_remains_liveness_only_when_qdrant_is_unavailable() {
    let app = TestApp::new(|settings| {
        settings.qdrant_url = "http://127.0.0.1:9".to_string();
    })
    .await;

    let response = app.raw_get("/api/health").await;

    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["status"], "ok");
}

#[tokio::test]
async fn jobs_api_exposes_index_job_snapshots_and_events() {
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png").unwrap();
    })
    .await;
    write_pattern_image(
        &app.source_path("job-index.png"),
        32,
        32,
        [180, 40, 40],
        [20, 20, 20],
    );

    let started = app.post_json("/api/jobs/index", json!({})).await;
    let job_id = started["spec"]["id"].as_str().unwrap().to_string();
    assert_eq!(started["spec"]["kind"], "index.manual");

    let finished = app.wait_for_job_status(&job_id, &["Succeeded"]).await;
    assert_eq!(finished["status"], "Succeeded");
    assert_eq!(finished["metadata"]["indexed"], "1");
    assert_eq!(finished["metadata"]["failed"], "0");

    let jobs = app.get_json("/api/jobs").await;
    assert!(jobs
        .as_array()
        .unwrap()
        .iter()
        .any(|job| job["spec"]["id"] == job_id));

    let events = app.get_json(&format!("/api/jobs/{job_id}/events")).await;
    let events = events.as_array().unwrap();
    assert!(!events.is_empty());
    let mut previous_sequence = 0_u64;
    for event in events {
        let sequence = event["sequence"].as_u64().unwrap();
        assert!(sequence > previous_sequence);
        previous_sequence = sequence;
    }
    assert!(events.iter().any(|event| {
        event["kind"]["StatusChanged"]["status"] == "Queued"
            || event["kind"]["StatusChanged"]["status"] == "Running"
            || event["kind"]["StatusChanged"]["status"] == "Succeeded"
    }));
    assert!(events
        .iter()
        .any(|event| event["kind"].get("Progress").is_some()));
    assert!(events.iter().any(|event| {
        event["kind"]["Progress"]["message"]
            .as_str()
            .map(|message| {
                message.contains("indexing source 1/1") && message.contains("job-index.png")
            })
            .unwrap_or(false)
    }));
    assert!(events.iter().any(|event| {
        event["kind"]["Progress"]["message"]
            .as_str()
            .map(|message| message.contains("(qdrant 7/7)"))
            .unwrap_or(false)
    }));
    assert!(events
        .iter()
        .any(|event| event["kind"].get("Log").is_some()));

    let fetched = app.get_json(&format!("/api/jobs/{job_id}")).await;
    assert_eq!(fetched["spec"]["id"], job_id);
}

#[tokio::test]
async fn async_index_job_indexes_static_pictures_and_search_finds_results() {
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png,.jpg").unwrap();
        settings.default_search_limit = 5;
        settings.duplicate_hash_distance = 0;
    })
    .await;
    let red = app.source_path("async-red.png");
    let green = app.source_path("async-green.png");
    let blue = app.source_path("async-blue.jpg");
    write_pattern_image(&red, 64, 40, [220, 20, 20], [20, 20, 20]);
    write_pattern_image(&green, 48, 48, [20, 180, 80], [20, 20, 20]);
    write_pattern_image(&blue, 40, 64, [30, 70, 220], [20, 20, 20]);

    let started = app.post_json("/api/jobs/index", json!({})).await;
    let job_id = started["spec"]["id"].as_str().unwrap().to_string();
    assert_eq!(started["spec"]["kind"], "index.manual");

    let finished = app.wait_for_job_status(&job_id, &["Succeeded"]).await;
    assert_eq!(finished["status"], "Succeeded");
    assert_eq!(finished["metadata"]["indexed"], "3");
    assert_eq!(finished["metadata"]["failed"], "0");

    let response = app
        .search_upload(
            "async-red-query.png",
            "image/png",
            fs::read(&red).unwrap(),
            Some(3),
        )
        .await;
    assert_eq!(response.query_media_kind, "static_image");
    assert_eq!(response.count, 3);
    assert_eq!(response.results[0].image.filename, "async-red.png");
    assert_eq!(response.results[0].hash_distance, Some(0));
    assert!(response.results[0].near_duplicate);
    let filenames = response
        .results
        .iter()
        .map(|result| result.image.filename.as_str())
        .collect::<Vec<_>>();
    assert!(filenames.contains(&"async-green.png"), "{filenames:?}");
    assert!(filenames.contains(&"async-blue.jpg"), "{filenames:?}");
    assert!(response
        .results
        .iter()
        .all(|result| result.image.media_kind == "static_image"));
}

#[tokio::test]
async fn cancel_job_api_requests_cancellation_for_active_jobs() {
    let app = TestApp::new(|_| {}).await;
    let job_id = app.spawn_cancellable_job();

    let cancelled = app
        .post_json(&format!("/api/jobs/{job_id}/cancel"), json!({}))
        .await;
    assert!(matches!(
        cancelled["status"].as_str().unwrap(),
        "Cancelling" | "Cancelled"
    ));

    let finished = app.wait_for_job_status(&job_id, &["Cancelled"]).await;
    assert_eq!(finished["status"], "Cancelled");

    let events = app.get_json(&format!("/api/jobs/{job_id}/events")).await;
    assert!(events.as_array().unwrap().iter().any(|event| {
        event["kind"]["StatusChanged"]["status"] == "Cancelled"
            || event["kind"]["StatusChanged"]["status"] == "Cancelling"
    }));

    let missing = app
        .client
        .post(format!("{}/api/jobs/not-a-real-job/cancel", app.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn indexing_endpoints_reject_overlapping_index_jobs() {
    let app = TestApp::new(|_| {}).await;
    let job_id = app.spawn_cancellable_index_job();

    let async_index = app.raw_post_json("/api/jobs/index", json!({})).await;
    assert_eq!(async_index.status(), reqwest::StatusCode::CONFLICT);

    let sync_index = app.raw_post_json("/api/index", json!({})).await;
    assert_eq!(sync_index.status(), reqwest::StatusCode::CONFLICT);

    let _ = app
        .post_json(&format!("/api/jobs/{job_id}/cancel"), json!({}))
        .await;
    let finished = app.wait_for_job_status(&job_id, &["Cancelled"]).await;
    assert_eq!(finished["status"], "Cancelled");
}

#[tokio::test]
async fn audio_transcription_model_endpoints_use_native_model_bundles() {
    let app = TestApp::new(|settings| {
        settings.audio_transcription_enabled = true;
    })
    .await;
    app.cache_audio_transcription_bundle();

    let catalog = app.get_json("/api/models/audio-transcription").await;
    assert_eq!(catalog["enabled"], true);
    assert_eq!(catalog["provider"], "candle-whisper");
    assert_eq!(catalog["configured_model"], "openai/whisper-large-v3-turbo");
    assert_eq!(catalog["device"], "auto");
    assert_eq!(catalog["compute_type"], "automatic");
    assert_eq!(catalog["batch_chunks"], true);
    assert_eq!(catalog["max_batch_size"], 4);
    let model = catalog["models"].as_array().unwrap().first().unwrap();
    assert_eq!(model["id"], "openai/whisper-large-v3-turbo");
    assert_eq!(model["cached"], true);
    assert_eq!(model["configured"], true);

    let download = app
        .post_json(
            "/api/models/audio-transcription/download",
            json!({ "model": "openai/whisper-large-v3-turbo" }),
        )
        .await;
    let download_id = download["spec"]["id"].as_str().unwrap();
    let download_finished = app.wait_for_job_status(download_id, &["Succeeded"]).await;
    assert_eq!(download_finished["status"], "Succeeded");
    assert_eq!(
        download_finished["spec"]["metadata"]["model"],
        "openai/whisper-large-v3-turbo"
    );
    assert_eq!(
        download_finished["spec"]["metadata"]["provider"],
        "candle-whisper"
    );

    let enable = app
        .post_json(
            "/api/models/audio-transcription/enable",
            json!({ "model": "openai/whisper-large-v3-turbo" }),
        )
        .await;
    let enable_id = enable["spec"]["id"].as_str().unwrap();
    let enable_finished = app.wait_for_job_status(enable_id, &["Succeeded"]).await;
    assert_eq!(enable_finished["status"], "Succeeded");
    assert_eq!(enable_finished["metadata"]["enabled"], "true");
    assert_eq!(
        enable_finished["metadata"]["configured_model"],
        "openai/whisper-large-v3-turbo"
    );

    let invalid = app
        .raw_post_json(
            "/api/models/audio-transcription/enable",
            json!({ "model": "unknown-model" }),
        )
        .await;
    assert_eq!(invalid.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: Value = invalid.json().await.unwrap();
    assert_eq!(
        body["detail"],
        "Audio transcription model `unknown-model` does not match configured native ASR model `openai/whisper-large-v3-turbo`"
    );
}

#[tokio::test]
async fn enabling_audio_transcription_without_bundle_reports_setup_error() {
    let app = TestApp::new(|settings| {
        settings.audio_transcription_enabled = false;
    })
    .await;

    let enable = app
        .post_json("/api/models/audio-transcription/enable", json!({}))
        .await;
    let enable_id = enable["spec"]["id"].as_str().unwrap();
    let enable_finished = app.wait_for_job_status(enable_id, &["Failed"]).await;

    assert_eq!(enable_finished["status"], "Failed");
    assert!(enable_finished["failure"]["message"]
        .as_str()
        .unwrap()
        .contains("native ASR model bundle"));
}

#[tokio::test]
async fn generic_model_catalog_reports_runtime_roles() {
    let app = TestApp::new(|_| {}).await;

    let catalog = app.get_json("/api/models").await;
    let roles = catalog["models"]
        .as_array()
        .unwrap()
        .iter()
        .map(|model| model["role"].as_str().unwrap().to_string())
        .collect::<Vec<_>>();

    assert!(roles.contains(&"visual_embedding".to_string()));
    assert!(roles.contains(&"face_detection".to_string()));
    assert!(roles.contains(&"face_embedding".to_string()));
    assert!(roles.contains(&"audio_transcription".to_string()));
}

#[tokio::test]
async fn model_disable_updates_runtime_config_and_cancels_active_index_jobs() {
    let app = TestApp::new(|settings| {
        settings.face_analysis_enabled = true;
    })
    .await;
    let index_spec = JobSpec::new(
        format!("index.manual.{}", Uuid::new_v4()),
        "Cancellable index job",
    )
    .and_then(|spec| spec.with_kind("index.manual"))
    .unwrap();
    let index_job = app
        .state
        .jobs
        .spawn(index_spec, |context| loop {
            context.check_cancelled()?;
            std::thread::sleep(Duration::from_millis(5));
        })
        .unwrap();

    let disable = app
        .client
        .post(format!(
            "{}/api/models/face_detection/disable",
            app.base_url
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(disable.status(), reqwest::StatusCode::OK);
    let disable_job: Value = disable.json().await.unwrap();
    let disable_job_id = disable_job["spec"]["id"].as_str().unwrap();
    app.wait_for_job_status(disable_job_id, &["Succeeded"])
        .await;
    app.state
        .jobs
        .wait_for_terminal(
            std::slice::from_ref(&index_job.spec.id),
            Duration::from_secs(2),
        )
        .unwrap();

    let source_config = app.get_json("/api/source-config").await;
    assert_eq!(source_config["indexing"]["face_analysis_enabled"], false);
    let catalog = app.get_json("/api/models").await;
    let face_detection = catalog["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["role"] == "face_detection")
        .unwrap();
    assert_eq!(face_detection["active"], false);

    let snapshot = app
        .state
        .jobs
        .snapshot(&index_job.spec.id)
        .unwrap()
        .unwrap();
    assert_eq!(snapshot.status, jobs_core::JobStatus::Cancelled);
}

#[tokio::test]
async fn sample_corpus_showcase_files_can_be_indexed_and_searched_when_downloaded() {
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf();
    let corpus_dir = std::env::var("SAMPLE_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| repo_root.join("sample-images/showcase"));
    if !corpus_dir.join("ATTRIBUTION.md").exists() {
        eprintln!("skipping sample corpus e2e test; run `bun run sample:download` first");
        return;
    }
    if !has_tool("ffmpeg")
        || !has_tool("ffprobe")
        || !has_tool("pdfinfo")
        || !has_tool("pdftoppm")
        || !has_tool("pdftotext")
    {
        eprintln!("skipping sample corpus e2e test because ffmpeg/ffprobe/poppler is unavailable");
        return;
    }

    let manifest_path = repo_root.join("tests/fixtures/sample-corpus/manifest.json");
    let manifest: SampleCorpusManifest =
        serde_json::from_str(&fs::read_to_string(manifest_path).unwrap()).unwrap();
    let assets_by_id = manifest
        .assets
        .iter()
        .map(|asset| (asset.id.as_str(), asset))
        .collect::<BTreeMap<_, _>>();

    let app = TestApp::new(|settings| {
        settings.image_extensions =
            parse_extensions(".jpg,.jpeg,.png,.gif,.mp4,.ogg,.pdf").unwrap();
        settings.audio_extensions = parse_extensions(".ogg").unwrap();
        settings.pdf_extensions = parse_extensions(".pdf").unwrap();
        settings.default_search_limit = 12;
        settings.video_frame_stride = 60;
        settings.video_max_frames = Some(4);
        settings.gif_sample_frames = 8;
        settings.pdf_max_pages = 4;
        settings.pdf_summary_pages = 2;
        settings.ocr_enabled = false;
    })
    .await;

    for asset in manifest
        .assets
        .iter()
        .filter(|asset| asset.role == "source")
    {
        let source = corpus_dir.join(&asset.filename);
        assert!(
            source.exists(),
            "missing downloaded sample {}",
            source.display()
        );
        let file_name = Path::new(&asset.filename).file_name().unwrap();
        fs::copy(&source, app.source_dir.join(file_name)).unwrap();
    }

    let indexed = app.index().await;
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);
    assert!(indexed.indexed >= 5, "{indexed:?}");

    for search in &manifest.searches {
        let query = assets_by_id
            .get(search.query_asset.as_str())
            .expect("query asset should exist");
        let expected = assets_by_id
            .get(search.expected_top_match.as_str())
            .expect("expected match should exist");
        let query_path = corpus_dir.join(&query.filename);
        let expected_filename = Path::new(&expected.filename)
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        let response = app
            .search_upload(
                Path::new(&query.filename)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .as_ref(),
                sample_content_type(&query.kind),
                fs::read(&query_path).unwrap(),
                Some(12),
            )
            .await;
        assert!(
            response
                .results
                .iter()
                .any(|result| result.image.filename == expected_filename),
            "search `{}` did not include expected match `{expected_filename}` in {:?}",
            search.id,
            response.results
        );
    }
}

#[derive(Deserialize)]
struct SampleCorpusManifest {
    assets: Vec<SampleCorpusAsset>,
    searches: Vec<SampleCorpusSearch>,
}

#[derive(Deserialize)]
struct SampleCorpusAsset {
    id: String,
    kind: String,
    role: String,
    filename: String,
}

#[derive(Deserialize)]
struct SampleCorpusSearch {
    id: String,
    query_asset: String,
    expected_top_match: String,
}

fn sample_content_type(kind: &str) -> &'static str {
    match kind {
        "static_image" => "image/jpeg",
        "animated_gif" => "image/gif",
        "audio" => "audio/ogg",
        "video" => "video/mp4",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}
