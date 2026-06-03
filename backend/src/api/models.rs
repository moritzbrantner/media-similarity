use std::fs::{self, File};
use std::io::{Read, Write};
use std::sync::Arc;

use axum::extract::{Path as AxumPath, State};
use axum::Json;
use jobs_core::{JobArtifact, JobContext, JobError, JobProgress, JobSpec};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use text_transcripts::{WhisperCppModel, WhisperCppModelStore};
use uuid::Uuid;

use super::jobs::ApiJobSnapshot;
use super::{ApiError, AppState};
use crate::config::Settings;
use crate::workers::media::audio::whisper_model_is_cached;
use crate::workers::media::models::{
    audio_transcription_model_store, download_role_bundle, model_statuses, parse_whisper_cpp_model,
    ModelRole, ModelRuntimeStatus,
};

#[derive(Debug, Serialize)]
pub struct AudioTranscriptionModelsResponse {
    pub enabled: bool,
    pub configured_model: String,
    pub auto_download: bool,
    pub cache_dir: Option<String>,
    pub models: Vec<AudioTranscriptionModelResponse>,
}

#[derive(Debug, Serialize)]
pub struct AudioTranscriptionModelResponse {
    pub id: String,
    pub cached: bool,
    pub configured: bool,
}

#[derive(Debug, Serialize)]
pub struct ModelsResponse {
    pub models: Vec<ModelRuntimeStatus>,
}

pub async fn get_models(State(state): State<Arc<AppState>>) -> Json<ModelsResponse> {
    Json(ModelsResponse {
        models: model_statuses(&state.indexing_settings()),
    })
}

pub async fn audio_transcription_models(
    State(state): State<Arc<AppState>>,
) -> Json<AudioTranscriptionModelsResponse> {
    let store = audio_transcription_model_store(&state.settings);
    let configured_model = state.settings.audio_transcription_model.clone();
    let models = store
        .catalog()
        .models
        .into_iter()
        .map(|status| {
            let id = status.model.id().to_string();
            AudioTranscriptionModelResponse {
                cached: status.cached,
                configured: id.eq_ignore_ascii_case(&configured_model),
                id,
            }
        })
        .collect();

    Json(AudioTranscriptionModelsResponse {
        enabled: state.settings.audio_transcription_enabled,
        configured_model,
        auto_download: state.settings.audio_transcription_auto_download,
        cache_dir: state
            .settings
            .audio_transcription_cache_dir
            .as_ref()
            .map(|path| path.to_string_lossy().to_string()),
        models,
    })
}

#[derive(Debug, Deserialize)]
pub struct AudioTranscriptionModelJobRequest {
    pub model: Option<String>,
}

pub async fn download_model(
    State(state): State<Arc<AppState>>,
    AxumPath(role): AxumPath<String>,
    Json(request): Json<AudioTranscriptionModelJobRequest>,
) -> Result<Json<ApiJobSnapshot>, ApiError> {
    let role = role.parse::<ModelRole>().map_err(ApiError::bad_request)?;
    if role == ModelRole::AudioTranscription {
        return spawn_audio_transcription_download(state, request.model).map(Json);
    }

    let settings = state.indexing_settings();
    let spec = crate::workers::media::models::role_spec(role).map_err(ApiError::bad_request)?;
    if let Some(requested) = request.model.as_deref() {
        if requested != spec.name {
            return Err(ApiError::bad_request(format!(
                "Model role `{}` supports configured model `{}`",
                role.as_str(),
                spec.name
            )));
        }
    }
    let spec_for_job = spec.clone();
    let model_name = spec.name.clone();
    let spec = JobSpec::new(
        format!("model.download.{}.{}", role.as_str(), Uuid::new_v4()),
        format!("Download {}", role.label()),
    )
    .and_then(|job_spec| job_spec.with_kind("model.download"))
    .and_then(|job_spec| job_spec.with_metadata("role", role.as_str()))
    .and_then(|job_spec| job_spec.with_metadata("model", model_name))
    .map_err(ApiError::from_job)?;
    state
        .jobs
        .spawn(spec, move |context| {
            context.info(format!("downloading model bundle `{}`", spec_for_job.name))?;
            context.progress(
                JobProgress::new(0, Some(1))?
                    .unit("steps")?
                    .message("downloading model bundle"),
            )?;
            let bundle = download_role_bundle(role, &settings).map_err(job_failed)?;
            context.artifact(
                JobArtifact::new("manifest", format!("model bundle {}", bundle.manifest.name))
                    .kind("model-bundle")
                    .path(bundle.manifest_path()),
            )?;
            context.progress(
                JobProgress::new(1, Some(1))?
                    .unit("steps")?
                    .message("model bundle ready"),
            )?;
            Ok(())
        })
        .map(ApiJobSnapshot::from)
        .map(Json)
        .map_err(ApiError::from_job)
}

pub async fn download_all_models(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ApiJobSnapshot>, ApiError> {
    let indexing_settings = state.indexing_settings();
    let app_settings = state.settings.clone();
    let audio_model = requested_audio_transcription_model(&app_settings, None)?;
    let audio_store = audio_transcription_model_store(&app_settings);
    let bundle_roles = [
        ModelRole::VisualEmbedding,
        ModelRole::FaceDetection,
        ModelRole::FaceEmbedding,
    ];
    let total = bundle_roles.len() as u64 + 1;
    let spec = JobSpec::new(
        format!("model.download.all.{}", Uuid::new_v4()),
        "Download every model",
    )
    .and_then(|job_spec| job_spec.with_kind("model.download_all"))
    .and_then(|job_spec| job_spec.with_metadata("scope", "all"))
    .and_then(|job_spec| job_spec.with_metadata("audio_model", audio_model.id()))
    .map_err(ApiError::from_job)?;

    state
        .jobs
        .spawn(spec, move |context| {
            let mut completed = 0_u64;
            for role in bundle_roles {
                context.check_cancelled()?;
                context.info(format!("downloading {} model", role.label()))?;
                context.progress(
                    JobProgress::new(completed, Some(total))?
                        .unit("models")?
                        .message(format!("downloading {}", role.label())),
                )?;
                let bundle = download_role_bundle(role, &indexing_settings).map_err(job_failed)?;
                context.artifact(
                    JobArtifact::new(
                        format!("manifest-{}", role.as_str()),
                        format!("model bundle {}", bundle.manifest.name),
                    )
                    .kind("model-bundle")
                    .path(bundle.manifest_path()),
                )?;
                completed += 1;
                context.progress(
                    JobProgress::new(completed, Some(total))?
                        .unit("models")?
                        .message(format!("{} ready", role.label())),
                )?;
            }

            context.check_cancelled()?;
            context.info(format!(
                "downloading audio transcription model `{}`",
                audio_model.id()
            ))?;
            context.progress(
                JobProgress::new(completed, Some(total))?
                    .unit("models")?
                    .message("downloading Audio transcription"),
            )?;
            download_whisper_cpp_model(&context, audio_store, audio_model)?;
            completed += 1;
            context.progress(
                JobProgress::new(completed, Some(total))?
                    .unit("models")?
                    .message("every model ready"),
            )?;
            Ok(())
        })
        .map(ApiJobSnapshot::from)
        .map(Json)
        .map_err(ApiError::from_job)
}

pub async fn enable_model(
    State(state): State<Arc<AppState>>,
    AxumPath(role): AxumPath<String>,
    Json(request): Json<AudioTranscriptionModelJobRequest>,
) -> Result<Json<ApiJobSnapshot>, ApiError> {
    let role = role.parse::<ModelRole>().map_err(ApiError::bad_request)?;
    if role == ModelRole::AudioTranscription {
        return spawn_audio_transcription_enable(state, request.model).map(Json);
    }

    let settings = state.indexing_settings();
    let status = crate::workers::media::models::model_status(role, &settings);
    if !status.cached {
        return Err(ApiError::bad_request(format!(
            "model `{}` is not cached; download it before enabling",
            status.configured
        )));
    }
    let spec = JobSpec::new(
        format!("model.enable.{}.{}", role.as_str(), Uuid::new_v4()),
        format!("Enable {}", role.label()),
    )
    .and_then(|spec| spec.with_kind("model.enable"))
    .and_then(|spec| spec.with_metadata("role", role.as_str()))
    .and_then(|spec| spec.with_metadata("model", status.configured.clone()))
    .map_err(ApiError::from_job)?;
    state
        .jobs
        .spawn(spec, move |context| {
            context.info(format!("model role `{}` is ready", role.as_str()))?;
            context.metadata("enabled", "true")?;
            context.progress(
                JobProgress::new(1, Some(1))?
                    .unit("steps")?
                    .message("model enable check complete"),
            )?;
            Ok(())
        })
        .map(ApiJobSnapshot::from)
        .map(Json)
        .map_err(ApiError::from_job)
}

pub async fn download_audio_transcription_model(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AudioTranscriptionModelJobRequest>,
) -> Result<Json<ApiJobSnapshot>, ApiError> {
    spawn_audio_transcription_download(state, request.model).map(Json)
}

pub async fn enable_audio_transcription_model(
    State(state): State<Arc<AppState>>,
    Json(request): Json<AudioTranscriptionModelJobRequest>,
) -> Result<Json<ApiJobSnapshot>, ApiError> {
    spawn_audio_transcription_enable(state, request.model).map(Json)
}

fn spawn_audio_transcription_download(
    state: Arc<AppState>,
    requested: Option<String>,
) -> Result<ApiJobSnapshot, ApiError> {
    let model = requested_audio_transcription_model(&state.settings, requested.as_deref())?;
    let store = audio_transcription_model_store(&state.settings);
    let spec = model_job_spec("model.download", "Download whisper.cpp model", model)?;
    state
        .jobs
        .spawn(spec, move |context| {
            download_whisper_cpp_model(&context, store, model)?;
            Ok(())
        })
        .map(ApiJobSnapshot::from)
        .map_err(ApiError::from_job)
}

fn spawn_audio_transcription_enable(
    state: Arc<AppState>,
    requested: Option<String>,
) -> Result<ApiJobSnapshot, ApiError> {
    let model = requested_audio_transcription_model(&state.settings, requested.as_deref())?;
    let store = audio_transcription_model_store(&state.settings);
    let settings = state.settings.clone();
    let spec = model_job_spec("model.enable", "Enable whisper.cpp model", model)?;
    state
        .jobs
        .spawn(spec, move |context| {
            enable_whisper_cpp_model(&context, &settings, &store, model)?;
            Ok(())
        })
        .map(ApiJobSnapshot::from)
        .map_err(ApiError::from_job)
}

fn requested_audio_transcription_model(
    settings: &Settings,
    requested: Option<&str>,
) -> Result<WhisperCppModel, ApiError> {
    parse_whisper_cpp_model(requested.unwrap_or(&settings.audio_transcription_model))
        .map_err(ApiError::bad_request)
}

fn model_job_spec(kind: &str, name: &str, model: WhisperCppModel) -> Result<JobSpec, ApiError> {
    JobSpec::new(
        format!("{kind}.whisper.{}.{}", model.id(), Uuid::new_v4()),
        name,
    )
    .and_then(|spec| spec.with_kind(kind))
    .and_then(|spec| spec.with_metadata("provider", "whisper.cpp"))
    .and_then(|spec| spec.with_metadata("model", model.id()))
    .map_err(ApiError::from_job)
}

fn download_whisper_cpp_model(
    context: &JobContext,
    store: WhisperCppModelStore,
    model: WhisperCppModel,
) -> jobs_core::Result<()> {
    context.info(format!("checking whisper.cpp model `{}`", model.id()))?;
    context.progress(
        JobProgress::new(0, Some(1))?
            .unit("steps")?
            .message("checking model cache"),
    )?;
    context.check_cancelled()?;

    let model_path = store.model_path(model);
    if model_path.is_file() {
        context.info(format!(
            "whisper.cpp model `{}` is already cached",
            model.id()
        ))?;
        context.progress(
            JobProgress::new(1, Some(1))?
                .unit("steps")?
                .message("model already cached"),
        )?;
        context.artifact(
            JobArtifact::new("model", format!("whisper.cpp model {}", model.id()))
                .kind("model")
                .path(model_path),
        )?;
        return Ok(());
    }

    fs::create_dir_all(store.models_dir()).map_err(job_failed)?;
    let temp_path = model_path.with_extension(format!("{}.part", context.id()));
    if temp_path.exists() {
        fs::remove_file(&temp_path).map_err(job_failed)?;
    }

    context.info(format!("downloading whisper.cpp model `{}`", model.id()))?;
    let mut response = reqwest::blocking::get(model.download_url())
        .and_then(|response| response.error_for_status())
        .map_err(job_failed)?;
    let total_bytes = response.content_length().filter(|total| *total > 0);
    let mut output = File::create(&temp_path).map_err(job_failed)?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        context.check_cancelled()?;
        let read = response.read(&mut buffer).map_err(job_failed)?;
        if read == 0 {
            break;
        }
        output.write_all(&buffer[..read]).map_err(job_failed)?;
        hasher.update(&buffer[..read]);
        downloaded += read as u64;
        context.progress(bytes_progress(
            downloaded,
            total_bytes,
            format!("downloaded {} bytes", downloaded),
        )?)?;
    }
    output.flush().map_err(job_failed)?;

    let checksum = format!("{:x}", hasher.finalize());
    if checksum != model.checksum_sha256() {
        let _ = fs::remove_file(&temp_path);
        return Err(JobError::Failed(format!(
            "downloaded model `{}` failed checksum verification",
            model.id()
        )));
    }

    fs::rename(&temp_path, &model_path).map_err(job_failed)?;
    context.info(format!("downloaded whisper.cpp model `{}`", model.id()))?;
    context.artifact(
        JobArtifact::new("model", format!("whisper.cpp model {}", model.id()))
            .kind("model")
            .path(model_path),
    )?;
    Ok(())
}

fn enable_whisper_cpp_model(
    context: &JobContext,
    settings: &Settings,
    store: &WhisperCppModelStore,
    model: WhisperCppModel,
) -> jobs_core::Result<()> {
    context.info(format!("checking whisper.cpp model `{}`", model.id()))?;
    context.progress(
        JobProgress::new(0, Some(1))?
            .unit("steps")?
            .message("checking model settings"),
    )?;
    context.check_cancelled()?;

    if !whisper_model_is_cached(store, model) {
        return Err(JobError::Failed(format!(
            "whisper.cpp model `{}` is not cached; download it before enabling",
            model.id()
        )));
    }

    if !settings.audio_transcription_enabled {
        context
            .warn("AUDIO_TRANSCRIPTION_ENABLED is false; set it to true to use transcription")?;
    }
    if !settings
        .audio_transcription_model
        .eq_ignore_ascii_case(model.id())
    {
        context.warn(format!(
            "AUDIO_TRANSCRIPTION_MODEL is `{}`; set it to `{}` to make this the active model",
            settings.audio_transcription_model,
            model.id()
        ))?;
    }
    context.metadata("enabled", settings.audio_transcription_enabled.to_string())?;
    context.metadata(
        "configured_model",
        settings.audio_transcription_model.clone(),
    )?;
    context.progress(
        JobProgress::new(1, Some(1))?
            .unit("steps")?
            .message("model enable check complete"),
    )?;
    context.info(format!("finished enable check for `{}`", model.id()))?;
    Ok(())
}

fn bytes_progress(
    completed: u64,
    total: Option<u64>,
    message: impl Into<String>,
) -> jobs_core::Result<JobProgress> {
    let total = total.filter(|total| *total >= completed && *total > 0);
    let progress = JobProgress::new(completed, total)?
        .unit("bytes")?
        .message(message);
    progress.validate()?;
    Ok(progress)
}

fn job_failed(error: impl std::fmt::Display) -> JobError {
    JobError::Failed(error.to_string())
}
