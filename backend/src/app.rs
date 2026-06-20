use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use tracing_subscriber::EnvFilter;

use crate::api::AppState;
use crate::config::Settings;

mod lifecycle;
mod router;

pub use router::upload_body_limit_bytes;

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
    lifecycle::initialize_startup_index_job(app_state.clone(), &settings);
    lifecycle::initialize_background_services(app_state.clone());

    let app = router::build_app_router(static_dir.clone(), app_state.clone(), &settings);
    let addr: SocketAddr = settings.bind_addr.parse()?;
    tracing::info!(%addr, "starting image similarity service");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(lifecycle::shutdown_signal(app_state))
        .await?;
    Ok(())
}

fn static_dir() -> PathBuf {
    std::env::var("FRONTEND_DIST_DIR")
        .or_else(|_| std::env::var("STATIC_DIR"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("frontend/dist"))
}
