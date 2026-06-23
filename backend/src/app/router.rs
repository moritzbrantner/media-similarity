use std::path::PathBuf;
use std::sync::Arc;

use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post, put};
use axum::Router;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;

use crate::api::{
    album_results, audio_transcription_models, cancel_job, create_album, delete_album,
    delete_indexed_media_route, delete_indexed_sources_route, disable_model, download_all_models,
    download_audio_transcription_model, download_model, enable_audio_transcription_model,
    enable_model, get_job, get_job_events, get_models, get_source_config, get_workflows, health,
    index_images, inverse_index, list_albums, list_jobs, merge_people, merge_speakers,
    preview_album, ready, rename_person, rename_speaker, reset_workflows, search_face_upload,
    search_upload, spawn_index_job, update_album, update_indexed_media_tags_route,
    update_source_config, update_workflows, validate_workflows, AppState,
};
use crate::config::Settings;

pub fn build_app_router(
    static_dir: PathBuf,
    app_state: Arc<AppState>,
    settings: &Settings,
) -> Router {
    let router = Router::new()
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
        .route("/api/workflows", get(get_workflows).put(update_workflows))
        .route("/api/workflows/validate", post(validate_workflows))
        .route("/api/workflows/reset", post(reset_workflows))
        .route("/api/jobs", get(list_jobs))
        .route("/api/jobs/index", post(spawn_index_job))
        .route("/api/jobs/:job_id", get(get_job))
        .route("/api/jobs/:job_id/events", get(get_job_events))
        .route("/api/jobs/:job_id/cancel", post(cancel_job))
        .route("/api/models", get(get_models))
        .route("/api/models/download-all", post(download_all_models))
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
        .route(
            "/api/indexed-media/:id/tags",
            put(update_indexed_media_tags_route),
        )
        .route("/api/indexed-sources", delete(delete_indexed_sources_route))
        .route(
            "/api/search",
            post(search_upload).layer(DefaultBodyLimit::max(upload_body_limit_bytes(settings))),
        )
        .route(
            "/api/search/face",
            post(search_face_upload)
                .layer(DefaultBodyLimit::max(upload_body_limit_bytes(settings))),
        )
        .nest_service("/thumbnails", ServeDir::new(settings.thumbnail_dir.clone()))
        .nest_service("/uploads", ServeDir::new(settings.upload_dir.clone()))
        .layer(TraceLayer::new_for_http())
        .with_state(app_state.clone());

    if settings.frontend_serving_enabled {
        mount_frontend_routes(router, static_dir)
    } else {
        router
    }
}

pub fn upload_body_limit_bytes(settings: &Settings) -> usize {
    settings.max_upload_mb as usize * 1024 * 1024 + 64 * 1024
}

fn mount_frontend_routes(router: Router, static_dir: PathBuf) -> Router {
    router
        .nest_service("/static", ServeDir::new(static_dir.clone()))
        .route_service("/", ServeFile::new(static_dir.join("index.html")))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::net::SocketAddr;
    use std::sync::Arc;

    use axum::Router;
    use reqwest::StatusCode;
    use tokio::task::JoinHandle;

    use super::build_app_router;
    use crate::api::AppState;
    use crate::config::Settings;

    #[tokio::test]
    async fn disabled_frontend_serving_keeps_api_and_media_routes_available() {
        let server = RouterTestServer::new(|settings| {
            settings.frontend_serving_enabled = false;
        })
        .await;

        assert_eq!(server.get("/api/health").await, StatusCode::OK);
        assert_ne!(server.get("/api/ready").await, StatusCode::NOT_FOUND);
        assert_eq!(
            server.get_body("/thumbnails/example.jpg").await,
            (StatusCode::OK, "thumbnail".to_string())
        );
        assert_eq!(
            server.get_body("/uploads/example.txt").await,
            (StatusCode::OK, "upload".to_string())
        );
        assert_eq!(server.get("/").await, StatusCode::NOT_FOUND);
        assert_eq!(server.get("/static/app.js").await, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn enabled_frontend_serving_serves_frontend_routes() {
        let server = RouterTestServer::new(|settings| {
            settings.frontend_serving_enabled = true;
        })
        .await;

        assert_eq!(
            server.get_body("/").await,
            (StatusCode::OK, "frontend".to_string())
        );
        assert_eq!(
            server.get_body("/static/app.js").await,
            (StatusCode::OK, "static".to_string())
        );
    }

    struct RouterTestServer {
        address: SocketAddr,
        client: reqwest::Client,
        _server: JoinHandle<()>,
        _temp_dir: TestTempDir,
    }

    impl RouterTestServer {
        async fn new(configure: impl FnOnce(&mut Settings)) -> Self {
            let temp_dir = TestTempDir::new();
            let static_dir = temp_dir.path().join("frontend");
            let thumbnail_dir = temp_dir.path().join("thumbnails");
            let upload_dir = temp_dir.path().join("uploads");
            fs::create_dir_all(&static_dir).unwrap();
            fs::create_dir_all(&thumbnail_dir).unwrap();
            fs::create_dir_all(&upload_dir).unwrap();
            fs::write(static_dir.join("index.html"), "frontend").unwrap();
            fs::write(static_dir.join("app.js"), "static").unwrap();
            fs::write(thumbnail_dir.join("example.jpg"), "thumbnail").unwrap();
            fs::write(upload_dir.join("example.txt"), "upload").unwrap();

            let mut settings = Settings {
                thumbnail_dir,
                upload_dir,
                ..Settings::default()
            };
            configure(&mut settings);
            let app = build_app_router(
                static_dir,
                Arc::new(AppState::new(settings.clone())),
                &settings,
            );
            Self::spawn(app, temp_dir).await
        }

        async fn spawn(app: Router, temp_dir: TestTempDir) -> Self {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                axum::serve(listener, app).await.unwrap();
            });
            Self {
                address,
                client: reqwest::Client::new(),
                _server: server,
                _temp_dir: temp_dir,
            }
        }

        async fn get(&self, path: &str) -> StatusCode {
            self.client
                .get(format!("http://{}{}", self.address, path))
                .send()
                .await
                .unwrap()
                .status()
        }

        async fn get_body(&self, path: &str) -> (StatusCode, String) {
            let response = self
                .client
                .get(format!("http://{}{}", self.address, path))
                .send()
                .await
                .unwrap();
            let status = response.status();
            let body = response.text().await.unwrap();
            (status, body)
        }
    }

    impl Drop for RouterTestServer {
        fn drop(&mut self) {
            self._server.abort();
        }
    }

    struct TestTempDir {
        path: std::path::PathBuf,
    }

    impl TestTempDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "media-sim-router-test-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&path).unwrap();
            Self { path }
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}
