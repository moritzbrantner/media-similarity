use std::collections::BTreeMap;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode as AxumStatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, ImageBuffer, Rgb, RgbImage};
use jobs_core::{JobProgress, JobSpec};
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use serde_json::{json, Value};
use text_analysis_transcription::WhisperCppModel;
use tokio::net::TcpListener;
use uuid::Uuid;

use image_similarity_service::api::{
    audio_transcription_models, cancel_job, download_audio_transcription_model,
    enable_audio_transcription_model, get_job, get_job_events, get_source_config, health,
    index_images, list_jobs, search_upload, spawn_index_job, update_source_config, AppState,
};
use image_similarity_service::config::{parse_extensions, Settings};
use image_similarity_service::domain::models::{IndexResponse, SearchResponse};

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
    assert!(scene.end_seconds > scene.start_seconds);
    assert!(scene.count <= 3);
    assert!(scene.results.iter().all(|result| {
        result.image.full_video_url.is_some() && result.image.scene_clip_url.is_some()
    }));
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
    assert_eq!(
        initial["supported_source_types"]
            .as_array()
            .unwrap()
            .iter()
            .find(|source_type| source_type["kind"] == "minio")
            .unwrap()["implemented"],
        false
    );

    let updated = app
        .put_json(
            "/api/source-config",
            json!({
                "sources": [
                    format!("  {}  ", extra_source.display()),
                    "",
                    "minio://bucket/prefix",
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
    assert_eq!(updated["sources"][1]["status"], "not_implemented");
    assert_eq!(updated["sources"][2]["kind"], "video");
    assert_eq!(updated["sources"][2]["status"], "not_implemented");
    assert_eq!(updated["sources"][3]["kind"], "camera");
    assert_eq!(updated["sources"][3]["status"], "not_implemented");

    let persisted = fs::read_to_string(app.media_sources_file()).unwrap();
    assert!(persisted.contains("# Managed by image-similarity-service."));
    assert!(persisted.contains(&extra_source.to_string_lossy().to_string()));
    assert!(persisted.contains("minio://bucket/prefix"));

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
            media_sources_file: root.path().join("config/media-sources.txt"),
            vector_size: 32,
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

        let state = Arc::new(AppState::new(settings));
        let app = Router::new()
            .route("/api/health", get(health))
            .route("/api/index", post(index_images))
            .route(
                "/api/source-config",
                get(get_source_config).put(update_source_config),
            )
            .route("/api/jobs", get(list_jobs))
            .route("/api/jobs/index", post(spawn_index_job))
            .route("/api/jobs/:job_id", get(get_job))
            .route("/api/jobs/:job_id/events", get(get_job_events))
            .route("/api/jobs/:job_id/cancel", post(cancel_job))
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
            .route("/api/search", post(search_upload))
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
        let spec = JobSpec::new(
            format!("test.cancel.{}", Uuid::new_v4()),
            "Cancellable test job",
        )
        .and_then(|spec| spec.with_kind("test.cancel"))
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
        let (request_content_type, body) = multipart_body(filename, content_type, bytes);
        let mut url = format!("{}/api/search", self.base_url);
        if let Some(limit) = limit {
            url.push_str(&format!("?limit={limit}"));
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
        let response = self
            .client
            .get(format!("{}{}", self.base_url, path))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        response.json().await.unwrap()
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
}

#[derive(Default)]
struct FakeQdrantState {
    collections: BTreeMap<String, Value>,
    points: BTreeMap<(String, String), FakePoint>,
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

impl FakeQdrant {
    async fn spawn() -> Self {
        let state = Arc::new(Mutex::new(FakeQdrantState::default()));
        let app = Router::new()
            .route("/collections", get(fake_list_collections))
            .route(
                "/collections/:collection",
                get(fake_get_collection).put(fake_create_collection),
            )
            .route("/collections/:collection/points", put(fake_upsert_points))
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
            .with_state(state);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            base_url: format!("http://{addr}"),
        }
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
    state
        .lock()
        .unwrap()
        .collections
        .insert(collection, request.vectors);
    Json(json!({ "result": true }))
}

async fn fake_get_collection(
    AxumPath(collection): AxumPath<String>,
    State(state): State<Arc<Mutex<FakeQdrantState>>>,
) -> Result<Json<Value>, AxumStatusCode> {
    let state = state.lock().unwrap();
    let Some(vectors) = state.collections.get(&collection) else {
        return Err(AxumStatusCode::NOT_FOUND);
    };
    Ok(Json(json!({
        "result": {
            "config": {
                "params": {
                    "vectors": vectors
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
        let expected = condition
            .get("match")
            .and_then(|value| value.get("value"))
            .and_then(Value::as_str);
        match expected {
            Some(expected) => payload.get(key).and_then(Value::as_str) == Some(expected),
            None => true,
        }
    })
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
