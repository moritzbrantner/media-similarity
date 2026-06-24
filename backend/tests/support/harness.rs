use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post, put};
use axum::Router;
use jobs_core::{JobProgress, JobSpec};
use reqwest::header::CONTENT_TYPE;
use serde_json::Value;
use text_transcripts::WhisperCppModel;
use tokio::net::TcpListener;
use uuid::Uuid;

use image_similarity_service::api::{
    album_results, audio_transcription_models, cancel_job, create_album, delete_album,
    delete_indexed_media_route, delete_indexed_sources_route, disable_model,
    download_audio_transcription_model, download_model, enable_audio_transcription_model,
    enable_model, get_job, get_job_events, get_models, get_source_config, health, index_images,
    inverse_index, list_albums, list_jobs, merge_people, merge_speakers, preview_album, ready,
    rename_person, rename_speaker, search_upload, spawn_index_job, update_album,
    update_source_config, AppState,
};
use image_similarity_service::app::upload_body_limit_bytes;
use image_similarity_service::config::Settings;
use image_similarity_service::domain::models::{ImagePayload, IndexResponse, SearchResponse};

use super::fake_qdrant::FakeQdrant;

pub struct TestApp {
    pub base_url: String,
    pub client: reqwest::Client,
    pub state: Arc<AppState>,
    pub source_dir: PathBuf,
    qdrant: FakeQdrant,
    root: TempDir,
}

impl TestApp {
    pub async fn new(configure: impl FnOnce(&mut Settings)) -> Self {
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
            .route("/api/inverse-index", get(inverse_index))
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
            .route("/api/models/:role/disable", post(disable_model))
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
            .route("/api/indexed-media/:id", delete(delete_indexed_media_route))
            .route("/api/indexed-sources", delete(delete_indexed_sources_route))
            .route(
                "/api/search",
                post(search_upload).layer(DefaultBodyLimit::max(search_body_limit)),
            )
            .with_state(state.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr: SocketAddr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        Self {
            base_url: format!("http://{addr}"),
            client: reqwest::Client::new(),
            state,
            source_dir,
            qdrant,
            root,
        }
    }

    pub fn source_path(&self, name: &str) -> PathBuf {
        self.source_dir.join(name)
    }

    pub fn root_path(&self) -> &Path {
        self.root.path()
    }

    pub fn media_sources_file(&self) -> &Path {
        &self.state.settings.media_sources_file
    }

    pub fn stored_media_payloads(&self) -> Vec<ImagePayload> {
        self.qdrant
            .media_payloads(&self.state.settings.qdrant_collection)
    }

    pub fn qdrant_payload_schema(&self) -> std::collections::BTreeMap<String, Value> {
        self.qdrant
            .payload_schema(&self.state.settings.qdrant_collection)
    }

    pub fn spawn_cancellable_job(&self) -> String {
        self.spawn_cancellable_job_with_kind("test.cancel")
    }

    pub fn spawn_cancellable_index_job(&self) -> String {
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

    pub fn cache_whisper_model(&self, model: WhisperCppModel) {
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

    pub async fn index(&self) -> IndexResponse {
        let response = self
            .client
            .post(format!("{}/api/index", self.base_url))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        response.json().await.unwrap()
    }

    pub async fn search_upload(
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

    pub async fn raw_search_upload(
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

    pub async fn search_upload_with_params(
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

    pub async fn raw_search_upload_with_params(
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

    pub async fn get_json(&self, path: &str) -> Value {
        let response = self.raw_get(path).await;
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        response.json().await.unwrap()
    }

    pub async fn raw_get(&self, path: &str) -> reqwest::Response {
        self.client
            .get(format!("{}{}", self.base_url, path))
            .send()
            .await
            .unwrap()
    }

    pub async fn post_json(&self, path: &str, body: Value) -> Value {
        let response = self.raw_post_json(path, body).await;
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        response.json().await.unwrap()
    }

    pub async fn raw_post_json(&self, path: &str, body: Value) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, path))
            .json(&body)
            .send()
            .await
            .unwrap()
    }

    pub async fn put_json(&self, path: &str, body: Value) -> Value {
        let response = self.raw_put_json(path, body).await;
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        response.json().await.unwrap()
    }

    pub async fn raw_put_json(&self, path: &str, body: Value) -> reqwest::Response {
        self.client
            .put(format!("{}{}", self.base_url, path))
            .json(&body)
            .send()
            .await
            .unwrap()
    }

    pub async fn delete_json(&self, path: &str) -> Value {
        let response = self
            .client
            .delete(format!("{}{}", self.base_url, path))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::OK);
        response.json().await.unwrap()
    }

    pub async fn wait_for_job_status(&self, job_id: &str, statuses: &[&str]) -> Value {
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
