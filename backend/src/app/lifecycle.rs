use std::sync::Arc;

use crate::api::AppState;
use crate::config::Settings;
use crate::workers::watcher::spawn_local_source_watcher;

pub fn initialize_background_services(app_state: Arc<AppState>) {
    let _source_watcher = spawn_local_source_watcher(app_state);
}

pub fn initialize_startup_index_job(app_state: Arc<AppState>, settings: &Settings) {
    if settings.startup_indexing_enabled {
        match crate::api::spawn_startup_index_job(app_state) {
            Ok(job) => tracing::info!(job_id = %job.spec.id, "queued startup indexing job"),
            Err(error) => tracing::warn!(%error, "could not queue startup indexing job"),
        }
    } else {
        tracing::info!("startup indexing is disabled");
    }
}

pub async fn shutdown_signal(state: Arc<AppState>) {
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
                jobs.wait_for_terminal(&ids, std::time::Duration::from_secs(30))
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
