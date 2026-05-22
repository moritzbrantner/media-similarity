use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::api::{
    audio_transcription_models, cancel_job, download_audio_transcription_model,
    enable_audio_transcription_model, get_job, get_job_events, get_source_config, health,
    index_images, list_jobs, search_upload, spawn_index_job, spawn_startup_index_job,
    update_source_config, AppState,
};
use crate::config::Settings;

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let settings =
        Settings::from_env().map_err(|error| format!("invalid configuration: {error}"))?;
    std::fs::create_dir_all(&settings.thumbnail_dir)?;
    std::fs::create_dir_all(&settings.upload_dir)?;

    let static_dir = static_dir();
    let app_state = Arc::new(AppState::new(settings.clone()));
    match spawn_startup_index_job(app_state.clone()) {
        Ok(job) => tracing::info!(job_id = %job.spec.id, "queued startup indexing job"),
        Err(error) => tracing::warn!(%error, "could not queue startup indexing job"),
    }
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
        .nest_service("/static", ServeDir::new(static_dir.clone()))
        .nest_service("/thumbnails", ServeDir::new(settings.thumbnail_dir.clone()))
        .nest_service("/uploads", ServeDir::new(settings.upload_dir.clone()))
        .route_service("/", ServeFile::new(static_dir.join("index.html")))
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let addr: SocketAddr = settings.bind_addr.parse()?;
    tracing::info!(%addr, "starting image similarity service");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn static_dir() -> PathBuf {
    std::env::var("FRONTEND_DIST_DIR")
        .or_else(|_| std::env::var("STATIC_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("frontend/dist"))
}
