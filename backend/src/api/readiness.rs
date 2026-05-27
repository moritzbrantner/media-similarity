use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use uuid::Uuid;

use super::{source_config_source, AppState};
use crate::workers::media::models::{model_statuses, ModelRuntimeStatus};

#[derive(Debug, Serialize)]
pub struct ReadinessResponse {
    pub status: String,
    pub checks: Vec<ReadinessCheck>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ReadinessCheck {
    pub name: String,
    pub status: String,
    pub detail: Option<String>,
}

impl ReadinessCheck {
    fn ok(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: "ok".to_string(),
            detail: Some(detail.into()),
        }
    }

    fn warn(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: "warn".to_string(),
            detail: Some(detail.into()),
        }
    }

    fn error(name: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: "error".to_string(),
            detail: Some(detail.into()),
        }
    }
}

pub async fn ready(State(state): State<Arc<AppState>>) -> Response {
    let mut checks = Vec::new();
    checks.push(qdrant_check(&state).await);

    let settings = state.indexing_settings();
    checks.push(writable_dir_check("thumbnail_dir", &settings.thumbnail_dir));
    checks.push(writable_dir_check("upload_dir", &settings.upload_dir));
    checks.push(media_sources_check(&state));
    checks.extend(model_checks(&settings));
    checks.push(command_check("ffmpeg", "ffmpeg", &["-version"], false));
    checks.push(command_check("ffprobe", "ffprobe", &["-version"], false));
    checks.push(poppler_check());
    checks.push(ocr_check(&settings));

    let response = readiness_response(checks);
    let status = if response.status == "ready" {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(response)).into_response()
}

async fn qdrant_check(state: &AppState) -> ReadinessCheck {
    match state.store.ensure_collection().await {
        Ok(()) => ReadinessCheck::ok(
            "qdrant",
            format!(
                "collection {} is available",
                state.settings.qdrant_collection
            ),
        ),
        Err(error) => ReadinessCheck::error("qdrant", error),
    }
}

fn writable_dir_check(name: &str, path: &Path) -> ReadinessCheck {
    if let Err(error) = fs::create_dir_all(path) {
        return ReadinessCheck::error(
            name,
            format!("could not create {}: {error}", path.display()),
        );
    }

    let probe = path.join(format!(
        ".readiness-writable-{}-{}",
        std::process::id(),
        Uuid::new_v4()
    ));
    match fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&probe)
    {
        Ok(_) => {
            let _ = fs::remove_file(probe);
            ReadinessCheck::ok(name, format!("{} is writable", path.display()))
        }
        Err(error) => ReadinessCheck::error(
            name,
            format!(
                "could not write readiness probe in {}: {error}",
                path.display()
            ),
        ),
    }
}

fn media_sources_check(state: &AppState) -> ReadinessCheck {
    let settings = state.indexing_settings();
    let sources = settings
        .source_specs()
        .into_iter()
        .map(|spec| source_config_source(spec, &settings))
        .collect::<Vec<_>>();
    let ready = sources
        .iter()
        .filter(|source| source.status == "ready")
        .count();

    if ready == sources.len() && ready > 0 {
        return ReadinessCheck::ok("media_sources", format!("{ready} source(s) ready"));
    }
    if ready > 0 {
        return ReadinessCheck::warn(
            "media_sources",
            format!("{ready}/{} source(s) ready", sources.len()),
        );
    }

    ReadinessCheck::error("media_sources", "no configured media source is ready")
}

fn model_checks(settings: &crate::config::Settings) -> Vec<ReadinessCheck> {
    model_statuses(settings)
        .into_iter()
        .map(model_check)
        .collect()
}

fn model_check(status: ModelRuntimeStatus) -> ReadinessCheck {
    let name = format!("model.{}", status.role);

    if status.active {
        return ReadinessCheck::ok(
            name,
            status
                .detail
                .unwrap_or_else(|| format!("{} is active", status.label)),
        );
    }

    if role_disabled(&status) {
        return ReadinessCheck::ok(
            name,
            status
                .detail
                .unwrap_or_else(|| "Role is disabled by configuration".to_string()),
        );
    }

    if status.blocking {
        return ReadinessCheck::error(name, missing_required_model_detail(&status));
    }

    ReadinessCheck::warn(name, missing_optional_model_detail(&status))
}

fn role_disabled(status: &ModelRuntimeStatus) -> bool {
    status
        .detail
        .as_deref()
        .map(|detail| detail == "Role is disabled by configuration")
        .unwrap_or(false)
}

fn missing_required_model_detail(status: &ModelRuntimeStatus) -> String {
    format!(
        "{} model `{}` is enabled but not cached. Download the {} model from the Models panel or POST /api/models/{}/download.",
        status.label, status.configured, status.role, status.role,
    )
}

fn missing_optional_model_detail(status: &ModelRuntimeStatus) -> String {
    let base = status.detail.clone().unwrap_or_else(|| {
        format!(
            "{} model `{}` is not cached",
            status.label, status.configured
        )
    });
    format!("{base}; related analysis will be skipped or unavailable until the model is downloaded")
}

fn command_check(name: &str, command: &str, args: &[&str], required: bool) -> ReadinessCheck {
    match Command::new(command).args(args).output() {
        Ok(output) if output.status.success() => {
            ReadinessCheck::ok(name, format!("{command} is available"))
        }
        Ok(output) => {
            let detail = format!("{command} exited with {}", output.status);
            if required {
                ReadinessCheck::error(name, detail)
            } else {
                ReadinessCheck::warn(name, detail)
            }
        }
        Err(error) => {
            let detail = format!("{command} is unavailable: {error}");
            if required {
                ReadinessCheck::error(name, detail)
            } else {
                ReadinessCheck::warn(name, detail)
            }
        }
    }
}

fn poppler_check() -> ReadinessCheck {
    let missing = ["pdfinfo", "pdftoppm", "pdftotext"]
        .into_iter()
        .filter_map(|command| match Command::new(command).arg("-v").output() {
            Ok(_) => None,
            Err(error) => Some(format!("{command}: {error}")),
        })
        .collect::<Vec<_>>();
    if missing.is_empty() {
        ReadinessCheck::ok("poppler", "Poppler PDF commands are available")
    } else {
        ReadinessCheck::warn("poppler", missing.join("; "))
    }
}

fn ocr_check(settings: &crate::config::Settings) -> ReadinessCheck {
    if !settings.ocr_enabled {
        return ReadinessCheck::ok("ocr", "disabled");
    }
    command_check("ocr", &settings.ocr_command, &["--version"], false)
}

fn readiness_response(checks: Vec<ReadinessCheck>) -> ReadinessResponse {
    let status = if checks.iter().any(|check| check.status == "error") {
        "not_ready"
    } else {
        "ready"
    };
    ReadinessResponse {
        status: status.to_string(),
        checks,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use super::{model_checks, readiness_response, writable_dir_check, ReadinessCheck};
    use crate::config::Settings;

    #[test]
    fn readiness_status_is_not_ready_when_any_check_errors() {
        let response = readiness_response(vec![
            ReadinessCheck::ok("ok", "ok"),
            ReadinessCheck::warn("warn", "warn"),
            ReadinessCheck::error("error", "error"),
        ]);

        assert_eq!(response.status, "not_ready");
    }

    #[test]
    fn readiness_status_is_ready_with_warnings_only() {
        let response = readiness_response(vec![
            ReadinessCheck::ok("ok", "ok"),
            ReadinessCheck::warn("warn", "warn"),
        ]);

        assert_eq!(response.status, "ready");
    }

    #[test]
    fn model_check_errors_when_enabled_visual_embedding_is_missing() {
        let root = tempfile_dir();
        let settings = Settings {
            visual_embedding_enabled: true,
            model_bundle_dir: root.join("missing-bundles"),
            visual_embedding_model_path: root.join("missing-model.onnx"),
            visual_embedding_preprocessor_path: root.join("missing-preprocessor.json"),
            face_analysis_enabled: false,
            audio_transcription_enabled: false,
            ..Settings::default()
        };

        let checks = model_checks(&settings);
        let visual = check_named(&checks, "model.visual_embedding");

        assert_eq!(visual.status, "error");
        let detail = visual.detail.as_deref().unwrap_or_default();
        assert!(detail.contains("visual_embedding"));
        assert!(detail.contains("/api/models/visual_embedding/download"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn model_check_ok_when_visual_embedding_is_disabled() {
        let settings = Settings {
            visual_embedding_enabled: false,
            face_analysis_enabled: false,
            audio_transcription_enabled: false,
            ..Settings::default()
        };

        let checks = model_checks(&settings);
        let visual = check_named(&checks, "model.visual_embedding");

        assert_eq!(visual.status, "ok");
        assert!(visual
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("disabled"));
    }

    #[test]
    fn model_check_warns_when_enabled_face_models_are_missing() {
        let root = tempfile_dir();
        let settings = Settings {
            visual_embedding_enabled: false,
            face_analysis_enabled: true,
            face_detection_model_path: root.join("missing-face-detector.onnx"),
            face_embedding_model_path: root.join("missing-face-embedder.onnx"),
            model_bundle_dir: root.join("missing-bundles"),
            audio_transcription_enabled: false,
            ..Settings::default()
        };

        let checks = model_checks(&settings);

        assert_eq!(check_named(&checks, "model.face_detection").status, "warn");
        assert_eq!(check_named(&checks, "model.face_embedding").status, "warn");
        assert_eq!(readiness_response(checks).status, "ready");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn writable_dir_probe_creates_missing_directory_and_cleans_up() {
        let root = std::env::temp_dir().join(format!(
            "image-sim-readiness-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let dir = root.join("nested");

        let check = writable_dir_check("test_dir", &dir);

        assert_eq!(check.status, "ok");
        assert!(dir.is_dir());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 0);
        let _ = fs::remove_dir_all(root);
    }

    fn check_named<'a>(checks: &'a [ReadinessCheck], name: &str) -> &'a ReadinessCheck {
        checks
            .iter()
            .find(|check| check.name == name)
            .unwrap_or_else(|| panic!("missing readiness check {name}"))
    }

    fn tempfile_dir() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "image-sim-readiness-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }
}
