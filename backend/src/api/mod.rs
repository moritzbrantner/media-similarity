mod error;
mod health;
mod identities;
mod indexed_media;
mod indexing;
mod inverse_index;
mod jobs;
mod models;
mod readiness;
mod search;
mod smart_albums;
mod source_config;
mod state;
mod workflows;

pub use error::ApiError;
pub use health::health;
pub use identities::{merge_people, merge_speakers, rename_person, rename_speaker};
pub use indexed_media::{
    delete_indexed_media_route, delete_indexed_sources_route, update_indexed_media_tags_route,
};
pub(crate) use indexing::run_index_job;
pub use indexing::{index_images, spawn_index_job, spawn_startup_index_job};
pub use inverse_index::inverse_index;
pub use jobs::{cancel_job, get_job, get_job_events, list_jobs};
pub use models::{
    audio_transcription_models, disable_model, download_all_models,
    download_audio_transcription_model, download_model, enable_audio_transcription_model,
    enable_model, get_models,
};
pub use readiness::ready;
pub use search::search_upload;
pub use smart_albums::{
    album_results, create_album, delete_album, list_albums, preview_album, update_album,
};
pub(crate) use source_config::source_config_source;
pub use source_config::{get_source_config, update_source_config, EditableIndexingConfig};
pub use state::AppState;
pub use workflows::{get_workflows, reset_workflows, update_workflows, validate_workflows};
