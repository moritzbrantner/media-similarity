use std::sync::Arc;

use axum::extract::{Path as AxumPath, State};
use axum::Json;
use jobs_core::{JobArtifact, JobError, JobProgress, JobSpec};
use uuid::Uuid;

use crate::api::jobs::ApiJobSnapshot;
use crate::workers::media::models::{download_role_bundle, load_role_bundle, ModelRole};

use crate::api::{ApiError, AppState};

#[derive(Debug, serde::Deserialize)]
pub struct AudioTranscriptionModelJobRequest {
    pub model: Option<String>,
}

pub async fn download_model(
    State(state): State<Arc<AppState>>,
    AxumPath(role): AxumPath<String>,
    Json(request): Json<AudioTranscriptionModelJobRequest>,
) -> Result<Json<crate::api::jobs::ApiJobSnapshot>, ApiError> {
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
    let audio_model = requested_audio_transcription_model_id(&app_settings, None)?;
    let bundle_roles = [
        ModelRole::VisualEmbedding,
        ModelRole::FaceDetection,
        ModelRole::FaceEmbedding,
        ModelRole::AudioTranscription,
    ];
    let total = bundle_roles.len() as u64;
    let spec = JobSpec::new(
        format!("model.download.all.{}", Uuid::new_v4()),
        "Download every model",
    )
    .and_then(|job_spec| job_spec.with_kind("model.download_all"))
    .and_then(|job_spec| job_spec.with_metadata("scope", "all"))
    .and_then(|job_spec| job_spec.with_metadata("audio_model", audio_model))
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
    let jobs = state.jobs.clone();
    let state_for_job = Arc::clone(&state);
    jobs.spawn(spec, move |context| {
        context.info(format!("enabling model role `{}`", role.as_str()))?;
        state_for_job.set_model_role_enabled(role, true);
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

pub async fn disable_model(
    State(state): State<Arc<AppState>>,
    AxumPath(role): AxumPath<String>,
) -> Result<Json<ApiJobSnapshot>, ApiError> {
    let role = role.parse::<ModelRole>().map_err(ApiError::bad_request)?;
    spawn_disable_model(state, role).map(Json)
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

pub(crate) fn spawn_audio_transcription_download(
    state: Arc<AppState>,
    requested: Option<String>,
) -> Result<ApiJobSnapshot, ApiError> {
    let model = requested_audio_transcription_model_id(&state.settings, requested.as_deref())?;
    let settings = state.indexing_settings();
    let spec = model_job_spec("model.download", "Download native ASR model bundle", &model)?;
    state
        .jobs
        .spawn(spec, move |context| {
            context.info(format!("downloading native ASR model bundle `{model}`"))?;
            context.progress(
                JobProgress::new(0, Some(1))?
                    .unit("steps")?
                    .message("checking model bundle"),
            )?;
            let bundle = download_role_bundle(ModelRole::AudioTranscription, &settings)
                .map_err(job_failed)?;
            context.artifact(
                JobArtifact::new("manifest", format!("native ASR model bundle {}", model))
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
        .map_err(ApiError::from_job)
}

pub(crate) fn spawn_audio_transcription_enable(
    state: Arc<AppState>,
    requested: Option<String>,
) -> Result<ApiJobSnapshot, ApiError> {
    let model = requested_audio_transcription_model_id(&state.settings, requested.as_deref())?;
    let settings = state.indexing_settings();
    let spec = model_job_spec("model.enable", "Enable native ASR model bundle", &model)?;
    let jobs = state.jobs.clone();
    let state_for_job = Arc::clone(&state);
    jobs.spawn(spec, move |context| {
        context.info(format!("checking native ASR model bundle `{model}`"))?;
        context.progress(
            JobProgress::new(0, Some(1))?
                .unit("steps")?
                .message("checking model bundle"),
        )?;
        let bundle = load_role_bundle(ModelRole::AudioTranscription, &settings).map_err(|error| {
            job_failed(format!(
                "native ASR model bundle `{model}` is not cached or invalid: {error}; download it before enabling transcription"
            ))
        })?;
        context.artifact(
            JobArtifact::new("manifest", format!("native ASR model bundle {}", model))
                .kind("model-bundle")
                .path(bundle.manifest_path()),
        )?;
        state_for_job.set_model_role_enabled(ModelRole::AudioTranscription, true);
        context.metadata("enabled", "true")?;
        context.metadata("configured_model", model)?;
        context.metadata("provider", "candle-whisper")?;
        context.progress(
            JobProgress::new(1, Some(1))?
                .unit("steps")?
                .message("model enable check complete"),
        )?;
        Ok(())
    })
    .map(ApiJobSnapshot::from)
    .map_err(ApiError::from_job)
}

pub(crate) fn spawn_disable_model(
    state: Arc<AppState>,
    role: ModelRole,
) -> Result<ApiJobSnapshot, ApiError> {
    let status = crate::workers::media::models::model_status(role, &state.indexing_settings());
    let spec = JobSpec::new(
        format!("model.disable.{}.{}", role.as_str(), Uuid::new_v4()),
        format!("Disable {}", role.label()),
    )
    .and_then(|spec| spec.with_kind("model.disable"))
    .and_then(|spec| spec.with_metadata("role", role.as_str()))
    .and_then(|spec| spec.with_metadata("model", status.configured.clone()))
    .map_err(ApiError::from_job)?;
    let jobs = state.jobs.clone();
    let state_for_job = Arc::clone(&state);
    jobs.spawn(spec, move |context| {
        context.info(format!("disabling model role `{}`", role.as_str()))?;
        context.progress(
            JobProgress::new(0, Some(1))?
                .unit("steps")?
                .message("disabling model role"),
        )?;
        state_for_job.set_model_role_enabled(role, false);
        let cancelled = state_for_job.jobs.request_cancel_kind_prefix("index.")?;
        if !cancelled.is_empty() {
            context.warn(format!(
                "requested cancellation for {} active indexing job(s)",
                cancelled.len()
            ))?;
        }
        context.metadata("enabled", "false")?;
        context.metadata("cancelled_index_jobs", cancelled.len().to_string())?;
        context.progress(
            JobProgress::new(1, Some(1))?
                .unit("steps")?
                .message("model role disabled"),
        )?;
        Ok(())
    })
    .map(ApiJobSnapshot::from)
    .map_err(ApiError::from_job)
}

fn requested_audio_transcription_model_id(
    settings: &crate::config::Settings,
    requested: Option<&str>,
) -> Result<String, ApiError> {
    let configured = settings.audio_transcription_model.trim();
    let requested = requested.unwrap_or(configured).trim();
    if requested != configured {
        return Err(ApiError::bad_request(format!(
            "Audio transcription model `{requested}` does not match configured native ASR model `{configured}`"
        )));
    }
    Ok(configured.to_string())
}

fn model_job_spec(kind: &str, name: &str, model: &str) -> Result<JobSpec, ApiError> {
    JobSpec::new(
        format!(
            "{kind}.audio_transcription.{}.{}",
            model.replace(['/', '\\'], "_"),
            Uuid::new_v4()
        ),
        name,
    )
    .and_then(|spec| spec.with_kind(kind))
    .and_then(|spec| spec.with_metadata("provider", "candle-whisper"))
    .and_then(|spec| spec.with_metadata("model", model))
    .map_err(ApiError::from_job)
}

fn job_failed(error: impl std::fmt::Display) -> JobError {
    JobError::Failed(error.to_string())
}
