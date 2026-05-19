use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

mod api;
mod audio;
mod config;
mod embedder;
mod hashing;
mod image_io;
mod indexer;
mod media;
mod models;
mod qdrant;
mod search;
mod sources;
mod thumbnails;
mod video;
mod voice;

use crate::api::{health, index_images, search_upload, AppState};
use crate::config::Settings;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let app = Router::new()
        .route("/api/health", get(health))
        .route("/api/index", post(index_images))
        .route("/api/search", post(search_upload))
        .nest_service("/static", ServeDir::new(static_dir.join("static")))
        .nest_service("/thumbnails", ServeDir::new(settings.thumbnail_dir.clone()))
        .nest_service("/uploads", ServeDir::new(settings.upload_dir.clone()))
        .route_service(
            "/",
            ServeFile::new(static_dir.join("static").join("index.html")),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(app_state);

    let addr: SocketAddr = settings.bind_addr.parse()?;
    tracing::info!(%addr, "starting image similarity service");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn static_dir() -> PathBuf {
    std::env::var("STATIC_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("src/image_similarity"))
}
