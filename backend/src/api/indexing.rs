use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use jobs_core::{JobContext, JobError, JobSnapshot, JobSpec};
use uuid::Uuid;

use super::jobs::ApiJobSnapshot;
use super::{ApiError, AppState};
use crate::config::Settings;
use crate::domain::models::IndexResponse;
use crate::storage::MediaVectorStore;
use crate::workers::indexer::ImageIndexer;
use crate::workers::media::visual_embedding::VisualEmbeddingBackend;

pub async fn index_images(State(state): State<Arc<AppState>>) -> Json<IndexResponse> {
    let indexer = ImageIndexer::new(
        state.indexing_settings(),
        state.store.clone(),
        state.embedder.clone(),
    );
    Json(indexer.index_sources().await)
}

pub async fn spawn_index_job(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiJobSnapshot>, ApiError> {
    let spec = JobSpec::new(
        format!("index.manual.{}", Uuid::new_v4()),
        "Index media sources",
    )
    .and_then(|spec| spec.with_kind("index.manual"))
    .and_then(|spec| spec.with_metadata("collection", state.settings.qdrant_collection.clone()))
    .map_err(ApiError::from_job)?;
    let jobs = state.jobs.clone();
    let settings = state.indexing_settings();
    let store = state.store.clone();
    let embedder = state.embedder.clone();

    jobs.spawn(spec, move |context| {
        run_index_job(context, settings, store, embedder)
    })
    .map(ApiJobSnapshot::from)
    .map(Json)
    .map_err(ApiError::from_job)
}

pub fn spawn_startup_index_job(state: Arc<AppState>) -> jobs_core::Result<JobSnapshot> {
    let spec = JobSpec::new(
        format!("index.startup.{}", Uuid::new_v4()),
        "Index missing media on startup",
    )?
    .with_kind("index.startup")?
    .with_metadata("collection", state.settings.qdrant_collection.clone())?;
    let jobs = state.jobs.clone();
    let settings = state.indexing_settings();
    let store = state.store.clone();
    let embedder = state.embedder.clone();

    jobs.spawn(spec, move |context| {
        run_index_job(context, settings, store, embedder)
    })
}

pub(crate) fn run_index_job(
    context: JobContext,
    settings: Settings,
    store: Arc<dyn MediaVectorStore>,
    embedder: Arc<dyn VisualEmbeddingBackend>,
) -> jobs_core::Result<()> {
    context.info("checking indexed media sources")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(job_failed)?;
    let indexer = ImageIndexer::new(settings, store, embedder);
    let response = runtime.block_on(indexer.index_missing_sources(Some(&context)));

    for error in &response.errors {
        context.warn(error.clone())?;
    }
    if context.is_cancelled() {
        return Err(JobError::Cancelled);
    }
    if response.failed > 0 {
        return Err(JobError::Failed(format!(
            "indexing finished with {} failed source file(s)",
            response.failed
        )));
    }
    Ok(())
}

fn job_failed(error: impl std::fmt::Display) -> JobError {
    JobError::Failed(error.to_string())
}
