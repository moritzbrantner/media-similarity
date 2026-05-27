use std::collections::BTreeMap;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use axum::extract::{DefaultBodyLimit, Path as AxumPath, State};
use axum::http::StatusCode as AxumStatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, ImageBuffer, Rgb, RgbImage};
use jobs_core::{JobProgress, JobSpec};
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use serde_json::{json, Value};
use text_transcripts::WhisperCppModel;
use tokio::net::TcpListener;
use uuid::Uuid;

use image_similarity_service::api::{
    album_results, audio_transcription_models, cancel_job, create_album, delete_album,
    delete_indexed_media_route, delete_indexed_sources_route, download_audio_transcription_model,
    download_model, enable_audio_transcription_model, enable_model, get_job, get_job_events,
    get_models, get_source_config, health, index_images, list_albums, list_jobs, merge_people,
    merge_speakers, preview_album, ready, rename_person, rename_speaker, search_upload,
    spawn_index_job, update_album, update_source_config, AppState,
};
use image_similarity_service::app::upload_body_limit_bytes;
use image_similarity_service::config::{parse_extensions, Settings};
use image_similarity_service::domain::models::{
    AudioAnalysis, AudioRecognizedVoice, AudioSegmentGuess, FaceBoxPayload, FaceDetectionPayload,
    FacePointPayload, ImagePayload, IndexResponse, PersonSummary, SearchResponse,
};
use image_similarity_service::workers::media::voice::VoiceRegistry;

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
async fn index_skips_files_that_are_already_current() {
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
    assert_eq!(second.skipped, 1);
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
    assert_eq!(second.skipped, 1);
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
    assert_eq!(second.skipped, 1);

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
    let schema = app
        ._qdrant
        .payload_schema(&app.state.settings.qdrant_collection);
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
    assert!(!events.as_array().unwrap().is_empty());

    let fetched = app.get_json(&format!("/api/jobs/{job_id}")).await;
    assert_eq!(fetched["spec"]["id"], job_id);
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
async fn audio_transcription_model_endpoints_report_cache_and_spawn_cached_jobs() {
    let app = TestApp::new(|settings| {
        settings.audio_transcription_enabled = true;
        settings.audio_transcription_model = "tiny.en".to_string();
        settings.audio_transcription_cache_dir = Some(settings.source_image_dir.join("../whisper"));
    })
    .await;
    app.cache_whisper_model(WhisperCppModel::TinyEn);

    let catalog = app.get_json("/api/models/audio-transcription").await;
    assert_eq!(catalog["enabled"], true);
    assert_eq!(catalog["configured_model"], "tiny.en");
    let tiny = catalog["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["id"] == "tiny.en")
        .unwrap();
    assert_eq!(tiny["cached"], true);
    assert_eq!(tiny["configured"], true);

    let download = app
        .post_json(
            "/api/models/audio-transcription/download",
            json!({ "model": "tiny.en" }),
        )
        .await;
    let download_id = download["spec"]["id"].as_str().unwrap();
    let download_finished = app.wait_for_job_status(download_id, &["Succeeded"]).await;
    assert_eq!(download_finished["status"], "Succeeded");
    assert_eq!(download_finished["spec"]["metadata"]["model"], "tiny.en");

    let enable = app
        .post_json(
            "/api/models/audio-transcription/enable",
            json!({ "model": "tiny.en" }),
        )
        .await;
    let enable_id = enable["spec"]["id"].as_str().unwrap();
    let enable_finished = app.wait_for_job_status(enable_id, &["Succeeded"]).await;
    assert_eq!(enable_finished["status"], "Succeeded");
    assert_eq!(enable_finished["metadata"]["enabled"], "true");
    assert_eq!(enable_finished["metadata"]["configured_model"], "tiny.en");

    let invalid = app
        .raw_post_json(
            "/api/models/audio-transcription/enable",
            json!({ "model": "unknown-model" }),
        )
        .await;
    assert_eq!(invalid.status(), reqwest::StatusCode::BAD_REQUEST);
    let body: Value = invalid.json().await.unwrap();
    assert_eq!(body["detail"], "Unknown whisper.cpp model `unknown-model`");
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

struct TestApp {
    base_url: String,
    client: reqwest::Client,
    state: Arc<AppState>,
    source_dir: PathBuf,
    _qdrant: FakeQdrant,
    _root: TempDir,
}

impl TestApp {
    async fn new(configure: impl FnOnce(&mut Settings)) -> Self {
        let root = TempDir::new();
        let source_dir = root.path().join("sources");
        let thumbnail_dir = root.path().join("thumbnails");
        let upload_dir = root.path().join("uploads");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&thumbnail_dir).unwrap();
        fs::create_dir_all(&upload_dir).unwrap();

        let qdrant = FakeQdrant::spawn().await;
        let mut settings = Settings {
            source_image_dir: source_dir.clone(),
            qdrant_url: qdrant.base_url.clone(),
            qdrant_collection: format!("test-{}", Uuid::new_v4()),
            thumbnail_dir,
            upload_dir: upload_dir.clone(),
            voice_registry_path: root.path().join("recognized-voices.json"),
            smart_albums_file: root.path().join("smart-albums.json"),
            media_sources_file: root.path().join("config/media-sources.txt"),
            vector_size: 32,
            visual_embedding_backend: "legacy".to_string(),
            visual_embedding_vector_size: 32,
            face_embedding_vector_size: 32,
            default_search_limit: 10,
            duplicate_hash_distance: 8,
            ocr_enabled: false,
            image_sources: Vec::new(),
            ..Settings::default()
        };
        configure(&mut settings);
        fs::create_dir_all(&settings.thumbnail_dir).unwrap();
        fs::create_dir_all(&settings.upload_dir).unwrap();

        let search_body_limit = upload_body_limit_bytes(&settings);
        let state = Arc::new(AppState::new(settings));
        let app = Router::new()
            .route("/api/health", get(health))
            .route("/api/ready", get(ready))
            .route("/api/index", post(index_images))
            .route("/api/smart-albums", get(list_albums).post(create_album))
            .route("/api/smart-albums/preview", post(preview_album))
            .route(
                "/api/smart-albums/:album_id",
                put(update_album).delete(delete_album),
            )
            .route("/api/smart-albums/:album_id/results", get(album_results))
            .route(
                "/api/inverse-index",
                get(image_similarity_service::api::inverse_index),
            )
            .route("/api/identities/people/:person_id", put(rename_person))
            .route(
                "/api/identities/people/:target_person_id/merge",
                post(merge_people),
            )
            .route("/api/identities/speakers/:speaker_id", put(rename_speaker))
            .route(
                "/api/identities/speakers/:target_speaker_id/merge",
                post(merge_speakers),
            )
            .route(
                "/api/source-config",
                get(get_source_config).put(update_source_config),
            )
            .route("/api/jobs", get(list_jobs))
            .route("/api/jobs/index", post(spawn_index_job))
            .route("/api/jobs/:job_id", get(get_job))
            .route("/api/jobs/:job_id/events", get(get_job_events))
            .route("/api/jobs/:job_id/cancel", post(cancel_job))
            .route("/api/models", get(get_models))
            .route("/api/models/:role/download", post(download_model))
            .route("/api/models/:role/enable", post(enable_model))
            .route(
                "/api/models/audio-transcription",
                get(audio_transcription_models),
            )
            .route(
                "/api/models/audio-transcription/download",
                post(download_audio_transcription_model),
            )
            .route(
                "/api/models/audio-transcription/enable",
                post(enable_audio_transcription_model),
            )
            .route(
                "/api/indexed-media/:id",
                axum::routing::delete(delete_indexed_media_route),
            )
            .route(
                "/api/indexed-sources",
                axum::routing::delete(delete_indexed_sources_route),
            )
            .route(
                "/api/search",
                post(search_upload).layer(DefaultBodyLimit::max(search_body_limit)),
            )
            .with_state(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Self {
            base_url: format!("http://{addr}"),
            client: reqwest::Client::new(),
            state,
            source_dir,
            _qdrant: qdrant,
            _root: root,
        }
    }

    fn source_path(&self, name: &str) -> PathBuf {
        self.source_dir.join(name)
    }

    fn root_path(&self) -> &Path {
        self._root.path()
    }

    fn media_sources_file(&self) -> &Path {
        &self.state.settings.media_sources_file
    }

    fn spawn_cancellable_job(&self) -> String {
        self.spawn_cancellable_job_with_kind("test.cancel")
    }

    fn spawn_cancellable_index_job(&self) -> String {
        self.spawn_cancellable_job_with_kind("index.manual")
    }

    fn spawn_cancellable_job_with_kind(&self, kind: &str) -> String {
        let spec = JobSpec::new(format!("{kind}.{}", Uuid::new_v4()), "Cancellable test job")
            .and_then(|spec| spec.with_kind(kind))
            .unwrap();
        let snapshot = self
            .state
            .jobs
            .spawn(spec, |context| {
                context.info("waiting for cancellation")?;
                context.progress(
                    JobProgress::new(0, None)?
                        .unit("checks")?
                        .message("waiting for cancellation"),
                )?;
                loop {
                    context.check_cancelled()?;
                    std::thread::sleep(Duration::from_millis(10));
                }
            })
            .unwrap();
        snapshot.spec.id.to_string()
    }

    fn cache_whisper_model(&self, model: WhisperCppModel) {
        let cache_dir = self
            .state
            .settings
            .audio_transcription_cache_dir
            .as_ref()
            .unwrap()
            .join("models");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join(model.file_name()), b"cached model").unwrap();
    }

    async fn index(&self) -> IndexResponse {
        let response = self
            .client
            .post(format!("{}/api/index", self.base_url))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        response.json().await.unwrap()
    }

    async fn search_upload(
        &self,
        filename: &str,
        content_type: &str,
        bytes: Vec<u8>,
        limit: Option<u32>,
    ) -> SearchResponse {
        let response = self
            .raw_search_upload(filename, content_type, bytes, limit)
            .await;
        let status = response.status();
        if status != reqwest::StatusCode::OK {
            let body = response.text().await.unwrap_or_default();
            panic!("expected search upload to succeed, got {status}: {body}");
        }
        response.json().await.unwrap()
    }

    async fn raw_search_upload(
        &self,
        filename: &str,
        content_type: &str,
        bytes: Vec<u8>,
        limit: Option<u32>,
    ) -> reqwest::Response {
        let mut params = Vec::new();
        if let Some(limit) = limit {
            params.push(("limit".to_string(), limit.to_string()));
        }
        self.raw_search_upload_with_params(filename, content_type, bytes, params)
            .await
    }

    async fn search_upload_with_params(
        &self,
        filename: &str,
        content_type: &str,
        bytes: Vec<u8>,
        params: Vec<(&str, String)>,
    ) -> SearchResponse {
        let response = self
            .raw_search_upload_with_params(
                filename,
                content_type,
                bytes,
                params
                    .into_iter()
                    .map(|(key, value)| (key.to_string(), value))
                    .collect(),
            )
            .await;
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        response.json().await.unwrap()
    }

    async fn raw_search_upload_with_params(
        &self,
        filename: &str,
        content_type: &str,
        bytes: Vec<u8>,
        params: Vec<(String, String)>,
    ) -> reqwest::Response {
        let (request_content_type, body) = multipart_body(filename, content_type, bytes);
        let mut url = format!("{}/api/search", self.base_url);
        if !params.is_empty() {
            let query = params
                .into_iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join("&");
            url.push('?');
            url.push_str(&query);
        }
        self.client
            .post(url)
            .header(CONTENT_TYPE, request_content_type)
            .body(body)
            .send()
            .await
            .unwrap()
    }

    async fn get_json(&self, path: &str) -> Value {
        let response = self.raw_get(path).await;
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        response.json().await.unwrap()
    }

    async fn raw_get(&self, path: &str) -> reqwest::Response {
        self.client
            .get(format!("{}{}", self.base_url, path))
            .send()
            .await
            .unwrap()
    }

    async fn post_json(&self, path: &str, body: Value) -> Value {
        let response = self.raw_post_json(path, body).await;
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        response.json().await.unwrap()
    }

    async fn raw_post_json(&self, path: &str, body: Value) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, path))
            .json(&body)
            .send()
            .await
            .unwrap()
    }

    async fn put_json(&self, path: &str, body: Value) -> Value {
        let response = self.raw_put_json(path, body).await;
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        response.json().await.unwrap()
    }

    async fn raw_put_json(&self, path: &str, body: Value) -> reqwest::Response {
        self.client
            .put(format!("{}{}", self.base_url, path))
            .json(&body)
            .send()
            .await
            .unwrap()
    }

    async fn delete_json(&self, path: &str) -> Value {
        let response = self
            .client
            .delete(format!("{}{}", self.base_url, path))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        response.json().await.unwrap()
    }

    async fn wait_for_job_status(&self, job_id: &str, statuses: &[&str]) -> Value {
        for _ in 0..100 {
            let snapshot = self.get_json(&format!("/api/jobs/{job_id}")).await;
            if statuses
                .iter()
                .any(|status| snapshot["status"].as_str() == Some(*status))
            {
                return snapshot;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("job `{job_id}` did not reach one of {statuses:?}");
    }
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("image-sim-e2e-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

struct FakeQdrant {
    base_url: String,
    state: Arc<Mutex<FakeQdrantState>>,
}

#[derive(Default)]
struct FakeQdrantState {
    collections: BTreeMap<String, FakeCollection>,
    points: BTreeMap<(String, String), FakePoint>,
}

struct FakeCollection {
    vectors: Value,
    payload_schema: BTreeMap<String, Value>,
}

#[derive(Clone)]
struct FakePoint {
    vector: Vec<f32>,
    payload: Value,
}

#[derive(Deserialize)]
struct FakeCreateCollectionRequest {
    vectors: Value,
}

#[derive(Deserialize)]
struct FakeCreatePayloadIndexRequest {
    field_name: String,
    field_schema: Value,
}

#[derive(Deserialize)]
struct FakeUpsertRequest {
    points: Vec<FakeUpsertPoint>,
}

#[derive(Deserialize)]
struct FakeUpsertPoint {
    id: String,
    vector: Value,
    payload: Value,
}

#[derive(Deserialize)]
struct FakeSearchRequest {
    vector: Value,
    limit: u32,
    filter: Option<Value>,
}

#[derive(Deserialize)]
struct FakeScrollRequest {
    limit: u32,
    offset: Option<Value>,
    filter: Option<Value>,
}

#[derive(Deserialize)]
struct FakeDeleteRequest {
    points: Vec<String>,
}

#[derive(Deserialize)]
struct FakeSetPayloadRequest {
    payload: Value,
    points: Vec<String>,
}

impl FakeQdrant {
    async fn spawn() -> Self {
        let state = Arc::new(Mutex::new(FakeQdrantState::default()));
        let app = Router::new()
            .route("/collections", get(fake_list_collections))
            .route(
                "/collections/:collection",
                get(fake_get_collection).put(fake_create_collection),
            )
            .route(
                "/collections/:collection/index",
                put(fake_create_payload_index),
            )
            .route("/collections/:collection/points", put(fake_upsert_points))
            .route(
                "/collections/:collection/points/payload",
                post(fake_set_payload),
            )
            .route(
                "/collections/:collection/points/delete",
                post(fake_delete_points),
            )
            .route(
                "/collections/:collection/points/scroll",
                post(fake_scroll_points),
            )
            .route(
                "/collections/:collection/points/search",
                post(fake_search_points),
            )
            .with_state(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            base_url: format!("http://{addr}"),
            state,
        }
    }

    fn payload_schema(&self, collection: &str) -> BTreeMap<String, Value> {
        self.state
            .lock()
            .unwrap()
            .collections
            .get(collection)
            .map(|collection| collection.payload_schema.clone())
            .unwrap_or_default()
    }
}

async fn fake_list_collections(State(state): State<Arc<Mutex<FakeQdrantState>>>) -> Json<Value> {
    let state = state.lock().unwrap();
    let collections = state
        .collections
        .keys()
        .map(|name| json!({ "name": name }))
        .collect::<Vec<_>>();
    Json(json!({ "result": { "collections": collections } }))
}

async fn fake_create_collection(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
    Json(request): Json<FakeCreateCollectionRequest>,
) -> Json<Value> {
    state.lock().unwrap().collections.insert(
        collection,
        FakeCollection {
            vectors: request.vectors,
            payload_schema: BTreeMap::new(),
        },
    );
    Json(json!({ "result": true }))
}

async fn fake_create_payload_index(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
    Json(request): Json<FakeCreatePayloadIndexRequest>,
) -> Result<Json<Value>, AxumStatusCode> {
    if collection.starts_with("fail-payload-index") {
        return Err(AxumStatusCode::SERVICE_UNAVAILABLE);
    }
    let mut state = state.lock().unwrap();
    let Some(collection) = state.collections.get_mut(&collection) else {
        return Err(AxumStatusCode::NOT_FOUND);
    };
    let data_type = request
        .field_schema
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            request
                .field_schema
                .get("type")
                .and_then(Value::as_str)
                .map(str::to_string)
        })
        .ok_or(AxumStatusCode::UNPROCESSABLE_ENTITY)?;
    collection
        .payload_schema
        .insert(request.field_name, json!({ "data_type": data_type }));
    Ok(Json(json!({ "result": { "status": "completed" } })))
}

async fn fake_get_collection(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
) -> Result<Json<Value>, AxumStatusCode> {
    let state = state.lock().unwrap();
    let Some(collection) = state.collections.get(&collection) else {
        return Err(AxumStatusCode::NOT_FOUND);
    };
    Ok(Json(json!({
        "result": {
            "payload_schema": &collection.payload_schema,
            "config": {
                "params": {
                    "vectors": &collection.vectors
                }
            }
        }
    })))
}

async fn fake_upsert_points(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
    Json(request): Json<FakeUpsertRequest>,
) -> Result<Json<Value>, AxumStatusCode> {
    let mut state = state.lock().unwrap();
    if !state.collections.contains_key(&collection) {
        return Err(AxumStatusCode::NOT_FOUND);
    }
    for point in request.points {
        let vector = named_vector(&point.vector).ok_or(AxumStatusCode::UNPROCESSABLE_ENTITY)?;
        state.points.insert(
            (collection.clone(), point.id),
            FakePoint {
                vector,
                payload: point.payload,
            },
        );
    }
    Ok(Json(json!({ "result": { "status": "completed" } })))
}

async fn fake_set_payload(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
    Json(request): Json<FakeSetPayloadRequest>,
) -> Result<Json<Value>, AxumStatusCode> {
    let mut state = state.lock().unwrap();
    if !state.collections.contains_key(&collection) {
        return Err(AxumStatusCode::NOT_FOUND);
    }
    for id in request.points {
        let Some(point) = state.points.get_mut(&(collection.clone(), id)) else {
            continue;
        };
        point.payload = request.payload.clone();
    }
    Ok(Json(json!({ "result": { "status": "completed" } })))
}

async fn fake_delete_points(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
    Json(request): Json<FakeDeleteRequest>,
) -> Result<Json<Value>, AxumStatusCode> {
    let mut state = state.lock().unwrap();
    if !state.collections.contains_key(&collection) {
        return Err(AxumStatusCode::NOT_FOUND);
    }
    for id in request.points {
        state.points.remove(&(collection.clone(), id));
    }
    Ok(Json(json!({ "result": { "status": "completed" } })))
}

async fn fake_search_points(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
    Json(request): Json<FakeSearchRequest>,
) -> Result<Json<Value>, AxumStatusCode> {
    let state = state.lock().unwrap();
    if !state.collections.contains_key(&collection) {
        return Err(AxumStatusCode::NOT_FOUND);
    }
    let mut scored = state
        .points
        .iter()
        .filter(|((point_collection, _), _)| point_collection == &collection)
        .filter(|(_, point)| payload_matches_filter(&point.payload, request.filter.as_ref()))
        .map(|((_, id), point)| {
            json!({
                "id": id,
                "score": cosine_similarity(&named_vector(&request.vector).unwrap_or_default(), &point.vector),
                "payload": point.payload,
            })
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right["score"]
            .as_f64()
            .unwrap()
            .total_cmp(&left["score"].as_f64().unwrap())
    });
    scored.truncate(request.limit as usize);
    Ok(Json(json!({ "result": scored })))
}

async fn fake_scroll_points(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
    Json(request): Json<FakeScrollRequest>,
) -> Result<Json<Value>, AxumStatusCode> {
    let state = state.lock().unwrap();
    if !state.collections.contains_key(&collection) {
        return Err(AxumStatusCode::NOT_FOUND);
    }
    let offset = request.offset.as_ref().and_then(Value::as_str);
    let mut points = state
        .points
        .iter()
        .filter(|((point_collection, id), _)| {
            point_collection == &collection
                && offset.map(|offset| id.as_str() > offset).unwrap_or(true)
        })
        .filter(|(_, point)| payload_matches_filter(&point.payload, request.filter.as_ref()))
        .map(|((_, id), point)| {
            json!({
                "id": id,
                "payload": point.payload,
            })
        })
        .collect::<Vec<_>>();
    points.sort_by(|left, right| left["id"].as_str().cmp(&right["id"].as_str()));
    let limit = request.limit as usize;
    let next_page_offset = if points.len() > limit {
        points
            .get(limit - 1)
            .and_then(|point| point["id"].as_str())
            .map(|id| json!(id))
    } else {
        None
    };
    points.truncate(limit);
    Ok(Json(json!({
        "result": {
            "points": points,
            "next_page_offset": next_page_offset,
        }
    })))
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let len = left.len().min(right.len());
    let mut dot = 0.0_f32;
    let mut left_norm = 0.0_f32;
    let mut right_norm = 0.0_f32;
    for index in 0..len {
        dot += left[index] * right[index];
        left_norm += left[index] * left[index];
        right_norm += right[index] * right[index];
    }
    if left_norm == 0.0 || right_norm == 0.0 {
        0.0
    } else {
        dot / (left_norm.sqrt() * right_norm.sqrt())
    }
}

fn named_vector(value: &Value) -> Option<Vec<f32>> {
    if let Some(values) = value.as_array() {
        return values
            .iter()
            .map(|value| value.as_f64().map(|number| number as f32))
            .collect();
    }
    if let Some(vector) = value.get("vector") {
        return named_vector(vector);
    }
    for name in ["visual", "face"] {
        if let Some(vector) = value.get(name) {
            return named_vector(vector);
        }
    }
    None
}

fn payload_matches_filter(payload: &Value, filter: Option<&Value>) -> bool {
    let Some(filter) = filter else {
        return true;
    };
    let Some(must) = filter.get("must").and_then(Value::as_array) else {
        return true;
    };
    must.iter().all(|condition| {
        let Some(key) = condition.get("key").and_then(Value::as_str) else {
            return true;
        };
        let actual = payload_value(payload, key);
        if let Some(expected) = condition.get("match").and_then(|value| value.get("value")) {
            return actual.map(|actual| actual == expected).unwrap_or(false);
        }
        if let Some(range) = condition.get("range") {
            let Some(actual) = actual.and_then(Value::as_f64) else {
                return false;
            };
            if let Some(gte) = range.get("gte").and_then(Value::as_f64) {
                if actual < gte {
                    return false;
                }
            }
            if let Some(lte) = range.get("lte").and_then(Value::as_f64) {
                if actual > lte {
                    return false;
                }
            }
        }
        true
    })
}

fn payload_value<'a>(payload: &'a Value, key: &str) -> Option<&'a Value> {
    let mut value = payload;
    for part in key.split('.') {
        value = value.get(part)?;
    }
    Some(value)
}

fn test_media_payload(id: &str, filename: &str) -> ImagePayload {
    ImagePayload {
        id: id.to_string(),
        path: format!("/images/{filename}"),
        relative_path: filename.to_string(),
        filename: filename.to_string(),
        width: 100,
        height: 100,
        size_bytes: 1000,
        modified_at: 1.0,
        phash: "0000000000000000".to_string(),
        thumbnail_url: Some(format!("/thumbnails/{id}.jpg")),
        animated_thumbnail_url: None,
        media_kind: "static_image".to_string(),
        frame_count: None,
        duration_ms: None,
        full_video_url: None,
        full_audio_url: None,
        full_pdf_url: None,
        pdf_page_url: None,
        pdf_document_id: None,
        pdf_page_index: None,
        pdf_page_number: None,
        pdf_page_count: None,
        audio_analysis: None,
        ocr_text: String::new(),
        ocr_frames: Vec::new(),
        visual_embedding_model: Some("test".to_string()),
        faces: Vec::new(),
        people: Vec::new(),
        artifacts: Vec::new(),
        tags: Vec::new(),
        photo_metadata: None,
        scene_clip_url: None,
        scene_index: None,
        scene_start_frame: None,
        scene_end_frame: None,
        scene_start_seconds: None,
        scene_end_seconds: None,
        source_type: "local".to_string(),
        source_item_uri: Some(format!("local:///images/{filename}")),
        indexing_profile: Some("test".to_string()),
        source_uri: Some("local:///images".to_string()),
    }
}

fn test_face_detection(
    face_id: &str,
    media_id: &str,
    person_id: &str,
    label: Option<&str>,
    confidence: f32,
) -> FaceDetectionPayload {
    FaceDetectionPayload {
        face_id: face_id.to_string(),
        media_id: media_id.to_string(),
        frame_index: 0,
        bbox: FaceBoxPayload {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        confidence,
        person_id: Some(person_id.to_string()),
        person_label: label.map(ToOwned::to_owned),
    }
}

fn test_face_point(
    face_id: &str,
    media_id: &str,
    person_id: &str,
    label: Option<&str>,
    confidence: f32,
) -> FacePointPayload {
    FacePointPayload {
        face_id: face_id.to_string(),
        media_id: media_id.to_string(),
        frame_index: 0,
        bbox: FaceBoxPayload {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        confidence,
        person_id: person_id.to_string(),
        person_label: label.map(ToOwned::to_owned),
        source_uri: Some("local:///images".to_string()),
        source_item_uri: Some(format!("local:///images/{media_id}.jpg")),
    }
}

fn test_person_summary(
    person_id: &str,
    label: Option<&str>,
    face_count: u32,
    confidence: f32,
) -> PersonSummary {
    PersonSummary {
        person_id: person_id.to_string(),
        label: label.map(ToOwned::to_owned),
        face_count,
        media_count: 1,
        confidence,
    }
}

fn seed_voice_registry(app: &TestApp) {
    let sample_rate = 8_000;
    let samples = (0..sample_rate)
        .map(|index| {
            let phase = index as f32 * 2.0 * std::f32::consts::PI * 180.0 / sample_rate as f32;
            phase.sin() * 0.2
        })
        .collect::<Vec<_>>();
    let mut registry = VoiceRegistry::load(&app.state.settings).unwrap();
    let enrolled = registry
        .recognize_or_enroll(&samples, sample_rate)
        .expect("voice should enroll");
    assert_eq!(enrolled.id, "voice-0001");
    registry.save_if_changed().unwrap();
}

fn multipart_body(filename: &str, content_type: &str, bytes: Vec<u8>) -> (String, Vec<u8>) {
    let boundary = format!("boundary-{}", Uuid::new_v4());
    let mut body = Vec::new();
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    body.extend_from_slice(format!("Content-Type: {content_type}\r\n\r\n").as_bytes());
    body.extend_from_slice(&bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

fn write_pattern_image(path: &Path, width: u32, height: u32, a: [u8; 3], b: [u8; 3]) {
    let mut image = RgbImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let pixel = if (x / 8 + y / 8) % 2 == 0 { a } else { b };
            image.put_pixel(x, y, Rgb(pixel));
        }
    }
    image.save(path).unwrap();
}

fn inject_xmp_metadata(path: &Path, xmp: &str) {
    let original = fs::read(path).unwrap();
    assert_eq!(&original[..2], &[0xff, 0xd8]);
    let mut payload = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
    payload.extend_from_slice(xmp.as_bytes());
    let length = payload.len() + 2;
    let mut metadata_segment = vec![0xff, 0xe1];
    metadata_segment.extend_from_slice(&(length as u16).to_be_bytes());
    metadata_segment.extend_from_slice(&payload);

    let mut updated = original[..2].to_vec();
    updated.extend_from_slice(&metadata_segment);
    updated.extend_from_slice(&original[2..]);
    fs::write(path, updated).unwrap();
}

fn test_photo_xmp() -> &'static str {
    r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:tiff="http://ns.adobe.com/tiff/1.0/" xmlns:dc="http://purl.org/dc/elements/1.1/">
<rdf:Description xmp:CreateDate="2024-03-12T10:30:00Z" tiff:Make="Acme" tiff:Model="Pocket 7">
<dc:subject><rdf:Bag><rdf:li>Travel</rdf:li><rdf:li>Sunrise</rdf:li></rdf:Bag></dc:subject>
</rdf:Description>
</rdf:RDF>
</x:xmpmeta>"#
}

fn write_test_gif(path: &Path, colors: &[[u8; 3]], delay_ms: u32) {
    let file = fs::File::create(path).unwrap();
    let mut encoder = GifEncoder::new(file);
    encoder.set_repeat(Repeat::Infinite).unwrap();
    let frames = colors
        .iter()
        .map(|color| {
            let image = ImageBuffer::from_pixel(32, 24, Rgb(*color));
            Frame::from_parts(
                image::DynamicImage::ImageRgb8(image).to_rgba8(),
                0,
                0,
                Delay::from_numer_denom_ms(delay_ms, 1),
            )
        })
        .collect::<Vec<_>>();
    encoder.encode_frames(frames).unwrap();
}

fn write_test_pdf(path: &Path, page_texts: &[&str]) {
    let mut objects = Vec::<String>::new();
    let kids = (0..page_texts.len())
        .map(|index| format!("{} 0 R", 3 + index))
        .collect::<Vec<_>>()
        .join(" ");
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_string());
    objects.push(format!(
        "<< /Type /Pages /Kids [{kids}] /Count {} >>",
        page_texts.len()
    ));
    let font_object_id = 3 + page_texts.len();
    let first_content_id = font_object_id + 1;
    for index in 0..page_texts.len() {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 240 160] /Resources << /Font << /F1 {font_object_id} 0 R >> >> /Contents {} 0 R >>",
            first_content_id + index
        ));
    }
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string());
    for text in page_texts {
        let stream = format!(
            "BT /F1 18 Tf 24 92 Td ({}) Tj ET",
            text.replace('\\', "\\\\")
                .replace('(', "\\(")
                .replace(')', "\\)")
        );
        objects.push(format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            stream.len(),
            stream
        ));
    }

    let mut pdf = Vec::from("%PDF-1.4\n".as_bytes());
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
    }
    let xref_offset = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    fs::write(path, pdf).unwrap();
}

fn write_two_scene_video(path: &Path) {
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-v")
        .arg("error")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("color=c=red:s=64x48:d=1:r=12")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("color=c=blue:s=64x48:d=1:r=12")
        .arg("-filter_complex")
        .arg("[0:v][1:v]concat=n=2:v=1:a=0,format=yuv420p")
        .arg("-movflags")
        .arg("+faststart")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn write_voice_like_audio(path: &Path) {
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-v")
        .arg("error")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("sine=frequency=220:duration=2.4:sample_rate=16000")
        .arg("-filter:a")
        .arg("volume=0.35")
        .arg("-ac")
        .arg("1")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn has_tool(name: &str) -> bool {
    Command::new(name)
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
