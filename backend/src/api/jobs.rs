use std::collections::BTreeMap;
use std::sync::Arc;

use axum::extract::{Path as AxumPath, State};
use axum::Json;
use chrono::{DateTime, Utc};
use jobs_core::{
    JobArtifact, JobEvent, JobEventKind, JobFailure, JobId, JobLogEntry, JobLogLevel, JobProgress,
    JobSnapshot, JobSpec, JobStatus,
};
use serde::Serialize;

use super::{ApiError, AppState};

#[derive(Clone, Copy, Debug, Serialize)]
pub enum ApiJobStatus {
    Queued,
    Running,
    Cancelling,
    Succeeded,
    Failed,
    Cancelled,
}

impl From<JobStatus> for ApiJobStatus {
    fn from(status: JobStatus) -> Self {
        match status {
            JobStatus::Queued => Self::Queued,
            JobStatus::Running => Self::Running,
            JobStatus::Cancelling => Self::Cancelling,
            JobStatus::Succeeded => Self::Succeeded,
            JobStatus::Failed => Self::Failed,
            JobStatus::Cancelled => Self::Cancelled,
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub enum ApiJobLogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl From<JobLogLevel> for ApiJobLogLevel {
    fn from(level: JobLogLevel) -> Self {
        match level {
            JobLogLevel::Debug => Self::Debug,
            JobLogLevel::Info => Self::Info,
            JobLogLevel::Warn => Self::Warn,
            JobLogLevel::Error => Self::Error,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiJobLogEntry {
    timestamp: DateTime<Utc>,
    level: ApiJobLogLevel,
    message: String,
}

impl From<JobLogEntry> for ApiJobLogEntry {
    fn from(log: JobLogEntry) -> Self {
        Self {
            timestamp: log.timestamp,
            level: log.level.into(),
            message: log.message,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiJobSnapshot {
    pub spec: JobSpec,
    pub status: ApiJobStatus,
    pub progress: Option<JobProgress>,
    pub logs: Vec<ApiJobLogEntry>,
    pub artifacts: Vec<JobArtifact>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub failure: Option<JobFailure>,
    pub metadata: BTreeMap<String, String>,
}

impl From<JobSnapshot> for ApiJobSnapshot {
    fn from(snapshot: JobSnapshot) -> Self {
        Self {
            spec: snapshot.spec,
            status: snapshot.status.into(),
            progress: snapshot.progress,
            logs: snapshot.logs.into_iter().map(Into::into).collect(),
            artifacts: snapshot.artifacts,
            created_at: snapshot.created_at,
            started_at: snapshot.started_at,
            finished_at: snapshot.finished_at,
            failure: snapshot.failure,
            metadata: snapshot.metadata,
        }
    }
}

#[derive(Debug, Serialize)]
pub enum ApiJobEventKind {
    StatusChanged {
        status: ApiJobStatus,
        message: Option<String>,
    },
    Progress(JobProgress),
    Log(ApiJobLogEntry),
    Artifact(JobArtifact),
    Metadata {
        key: String,
        value: String,
    },
}

impl From<JobEventKind> for ApiJobEventKind {
    fn from(kind: JobEventKind) -> Self {
        match kind {
            JobEventKind::StatusChanged { status, message } => Self::StatusChanged {
                status: status.into(),
                message,
            },
            JobEventKind::Progress(progress) => Self::Progress(progress),
            JobEventKind::Log(log) => Self::Log(log.into()),
            JobEventKind::Artifact(artifact) => Self::Artifact(artifact),
            JobEventKind::Metadata { key, value } => Self::Metadata { key, value },
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiJobEvent {
    job_id: JobId,
    sequence: u64,
    timestamp: DateTime<Utc>,
    kind: ApiJobEventKind,
}

impl From<JobEvent> for ApiJobEvent {
    fn from(event: JobEvent) -> Self {
        Self {
            job_id: event.job_id,
            sequence: event.sequence,
            timestamp: event.timestamp,
            kind: event.kind.into(),
        }
    }
}

pub async fn list_jobs(
    State(state): State<Arc<AppState>>,
) -> Result<Json<Vec<ApiJobSnapshot>>, ApiError> {
    state
        .jobs
        .snapshots()
        .map(|jobs| jobs.into_iter().map(ApiJobSnapshot::from).collect())
        .map(Json)
        .map_err(ApiError::from_job)
}

pub async fn get_job(
    State(state): State<Arc<AppState>>,
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<ApiJobSnapshot>, ApiError> {
    let job_id = parse_job_id(job_id)?;
    state
        .jobs
        .snapshot(&job_id)
        .map_err(ApiError::from_job)?
        .map(ApiJobSnapshot::from)
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("Unknown job `{job_id}`")))
}

pub async fn get_job_events(
    State(state): State<Arc<AppState>>,
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<Vec<ApiJobEvent>>, ApiError> {
    let job_id = parse_job_id(job_id)?;
    if state
        .jobs
        .snapshot(&job_id)
        .map_err(ApiError::from_job)?
        .is_none()
    {
        return Err(ApiError::not_found(format!("Unknown job `{job_id}`")));
    }
    state
        .jobs
        .events(&job_id)
        .map(|events| events.into_iter().map(ApiJobEvent::from).collect())
        .map(Json)
        .map_err(ApiError::from_job)
}

pub async fn cancel_job(
    State(state): State<Arc<AppState>>,
    AxumPath(job_id): AxumPath<String>,
) -> Result<Json<ApiJobSnapshot>, ApiError> {
    let job_id = parse_job_id(job_id)?;
    state
        .jobs
        .request_cancel(&job_id)
        .map_err(ApiError::from_job)?;
    state
        .jobs
        .snapshot(&job_id)
        .map_err(ApiError::from_job)?
        .map(ApiJobSnapshot::from)
        .map(Json)
        .ok_or_else(|| ApiError::not_found(format!("Unknown job `{job_id}`")))
}

fn parse_job_id(value: String) -> Result<JobId, ApiError> {
    JobId::new(value).map_err(|error| ApiError::bad_request(error.to_string()))
}
