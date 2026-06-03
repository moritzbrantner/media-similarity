use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post, put};
use axum::Router;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use crate::api::{
    album_results, audio_transcription_models, cancel_job, create_album, delete_album,
    delete_indexed_media_route, delete_indexed_sources_route, download_all_models,
    download_audio_transcription_model, download_model, enable_audio_transcription_model,
    enable_model, get_job, get_job_events, get_models, get_source_config, get_workflows, health,
    index_images, inverse_index, list_albums, list_jobs, merge_people, merge_speakers,
    preview_album, ready, rename_person, rename_speaker, reset_workflows, search_upload,
    spawn_index_job, spawn_startup_index_job, update_album, update_indexed_media_tags_route,
    update_source_config, update_workflows, validate_workflows, AppState,
};
use crate::config::Settings;
use crate::workers::watcher::spawn_local_source_watcher;

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
    if settings.startup_indexing_enabled {
        match spawn_startup_index_job(app_state.clone()) {
            Ok(job) => tracing::info!(job_id = %job.spec.id, "queued startup indexing job"),
            Err(error) => tracing::warn!(%error, "could not queue startup indexing job"),
        }
    } else {
        tracing::info!("startup indexing is disabled");
    }
    let _source_watcher = spawn_local_source_watcher(app_state.clone());
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
            post(search_upload).layer(DefaultBodyLimit::max(upload_body_limit_bytes(&settings))),
        )
        .nest_service("/static", ServeDir::new(static_dir.clone()))
        .nest_service("/thumbnails", ServeDir::new(settings.thumbnail_dir.clone()))
        .nest_service("/uploads", ServeDir::new(settings.upload_dir.clone()))
        .route_service("/", ServeFile::new(static_dir.join("index.html")))
        .layer(TraceLayer::new_for_http())
        .with_state(app_state.clone());

    let addr: SocketAddr = settings.bind_addr.parse()?;
    tracing::info!(%addr, "starting image similarity service");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(app_state))
        .await?;
    Ok(())
}

async fn shutdown_signal(state: Arc<AppState>) {
    wait_for_shutdown_signal().await;
    tracing::info!("shutdown requested; cancelling active indexing jobs");
    match state.jobs.request_cancel_kind_prefix("index.") {
        Ok(ids) if ids.is_empty() => {
            tracing::info!("no active indexing jobs to cancel");
        }
        Ok(ids) => {
            let jobs = state.jobs.clone();
            let job_count = ids.len();
            match tokio::task::spawn_blocking(move || {
                jobs.wait_for_terminal(&ids, Duration::from_secs(30))
            })
            .await
            {
                Ok(Ok(())) => tracing::info!(job_count, "indexing jobs stopped before shutdown"),
                Ok(Err(error)) => {
                    tracing::warn!(%error, "shutdown is continuing with indexing jobs still active")
                }
                Err(error) => tracing::warn!(
                    %error,
                    "shutdown wait task failed; shutdown is continuing"
                ),
            }
        }
        Err(error) => tracing::warn!(%error, "could not request indexing job cancellation"),
    }
}

#[cfg(unix)]
async fn wait_for_shutdown_signal() {
    let ctrl_c = async {
        if let Err(error) = tokio::signal::ctrl_c().await {
            tracing::warn!(%error, "could not listen for Ctrl-C shutdown signal");
        }
    };
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut signal) => {
                signal.recv().await;
            }
            Err(error) => {
                tracing::warn!(%error, "could not listen for SIGTERM shutdown signal");
                std::future::pending::<()>().await;
            }
        }
    };
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[cfg(not(unix))]
async fn wait_for_shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::warn!(%error, "could not listen for Ctrl-C shutdown signal");
    }
}

fn static_dir() -> PathBuf {
    std::env::var("FRONTEND_DIST_DIR")
        .or_else(|_| std::env::var("STATIC_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("frontend/dist"))
}

pub fn upload_body_limit_bytes(settings: &Settings) -> usize {
    settings.max_upload_mb as usize * 1024 * 1024 + 64 * 1024
}
