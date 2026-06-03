use std::sync::{Arc, RwLock};

use crate::config::Settings;
use crate::storage::qdrant::{QdrantHttpOptions, QdrantImageStore};
use crate::storage::MediaVectorStore;
use crate::workers::jobs::JobManager;
use crate::workers::media::visual_embedding::{build_visual_embedder, VisualEmbeddingBackend};
use crate::workers::workflows::{
    compile_media_workflow, default_media_workflow_library, load_media_workflow_library,
    media_workflow_hash, validate_media_workflow_library, CompiledMediaWorkflow, MediaFileKind,
    MediaWorkflowLibrary, WorkflowMode,
};

use super::EditableIndexingConfig;

pub struct AppState {
    pub settings: Settings,
    indexing_config: RwLock<EditableIndexingConfig>,
    source_specs: RwLock<Vec<String>>,
    workflow_library: RwLock<MediaWorkflowLibrary>,
    pub store: Arc<dyn MediaVectorStore>,
    pub embedder: Arc<dyn VisualEmbeddingBackend>,
    pub jobs: JobManager,
}

impl AppState {
    pub fn new(settings: Settings) -> Self {
        let store = Arc::new(QdrantImageStore::new_with_options(
            settings.qdrant_url.clone(),
            settings.qdrant_collection.clone(),
            settings.visual_embedding_vector_size,
            settings.face_embedding_vector_size,
            QdrantHttpOptions {
                request_timeout_ms: settings.qdrant_request_timeout_ms,
                connect_timeout_ms: settings.qdrant_connect_timeout_ms,
                retry_attempts: settings.qdrant_retry_attempts,
                retry_backoff_ms: settings.qdrant_retry_backoff_ms,
            },
        ));
        let embedder = build_visual_embedder(&settings);
        let indexing_config = RwLock::new(EditableIndexingConfig::from_settings(&settings));
        let source_specs = RwLock::new(settings.source_specs());
        let workflow_library = RwLock::new(load_workflow_library_or_default(&settings));
        Self {
            settings,
            indexing_config,
            source_specs,
            workflow_library,
            store,
            embedder,
            jobs: JobManager::default(),
        }
    }

    pub fn indexing_settings(&self) -> Settings {
        let mut settings = self.settings.clone();
        settings.image_sources = read_source_specs(&self.source_specs);
        read_indexing_config(&self.indexing_config).apply_to_settings(&mut settings);
        let workflow_library = self.workflow_library();
        settings.processing_workflows_hash = Some(media_workflow_hash(&workflow_library));
        settings
    }

    pub fn workflow_library(&self) -> MediaWorkflowLibrary {
        self.workflow_library
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    pub fn compiled_workflow(
        &self,
        kind: MediaFileKind,
        mode: WorkflowMode,
    ) -> Result<CompiledMediaWorkflow, String> {
        compile_media_workflow(kind, mode, &self.workflow_library())
    }

    pub(super) fn replace_source_specs(&self, sources: Vec<String>) {
        let mut source_specs = self
            .source_specs
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *source_specs = sources;
    }

    pub(super) fn replace_indexing_config(&self, indexing_config: EditableIndexingConfig) {
        let mut current = self
            .indexing_config
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *current = indexing_config;
    }

    pub(crate) fn replace_workflow_library(&self, library: MediaWorkflowLibrary) {
        let mut current = self
            .workflow_library
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *current = library;
    }
}

fn load_workflow_library_or_default(settings: &Settings) -> MediaWorkflowLibrary {
    match load_media_workflow_library(&settings.processing_workflows_file) {
        Ok(library) => {
            let diagnostics = validate_media_workflow_library(&library);
            if diagnostics.is_empty() {
                library
            } else {
                tracing::warn!(
                    path = %settings.processing_workflows_file.display(),
                    diagnostics = ?diagnostics,
                    "processing workflow file is invalid; using defaults"
                );
                default_media_workflow_library(settings)
            }
        }
        Err(error) => {
            tracing::debug!(%error, "processing workflow file was not loaded; using defaults");
            default_media_workflow_library(settings)
        }
    }
}

fn read_indexing_config(
    indexing_config: &RwLock<EditableIndexingConfig>,
) -> EditableIndexingConfig {
    indexing_config
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}

fn read_source_specs(source_specs: &RwLock<Vec<String>>) -> Vec<String> {
    source_specs
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .clone()
}
