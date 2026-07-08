use crate::config::Settings;
use crate::workers::sources::{
    parse_media_source_spec, SourceCapabilities, SourceDiagnostic, SourceDiagnosticCode,
    SourceDiagnosticSeverity,
};

use super::contracts::{SourceConfigSource, SupportedSourceType};

pub(crate) fn source_config_source(spec: String, settings: &Settings) -> SourceConfigSource {
    let media_source = parse_media_source_spec(&spec);
    let kind = media_source.kind.as_str().to_string();
    let mut diagnostics = media_source.diagnostics;
    match kind.as_str() {
        "local" => {
            let path = local_source_path(&media_source.normalized_uri);
            if !path.is_dir() {
                diagnostics.push(SourceDiagnostic::error(
                    SourceDiagnosticCode::Unavailable,
                    format!("Directory does not exist: {}", path.display()),
                ));
            }
        }
        "minio" | "s3" => {
            diagnostics.extend(object_source_config_diagnostics(&spec, &kind, settings));
        }
        "video" | "camera" => {}
        _ => {}
    };
    let status = if matches!(kind.as_str(), "video" | "camera") {
        "not_implemented".to_string()
    } else {
        source_status(&diagnostics, &media_source.capabilities)
    };
    let detail = diagnostics
        .iter()
        .find(|diagnostic| matches!(diagnostic.severity, SourceDiagnosticSeverity::Error))
        .or_else(|| diagnostics.first())
        .map(|diagnostic| diagnostic.message.clone());

    SourceConfigSource {
        id: media_source.id,
        spec,
        normalized_uri: media_source.normalized_uri,
        kind,
        status,
        detail,
        diagnostics,
        capabilities: media_source.capabilities,
    }
}

pub(crate) fn supported_source_types() -> Vec<SupportedSourceType> {
    vec![
        SupportedSourceType {
            kind: "local".to_string(),
            label: "Local folder".to_string(),
            implemented: true,
            example: "/images or local:///images".to_string(),
        },
        SupportedSourceType {
            kind: "minio".to_string(),
            label: "MinIO bucket".to_string(),
            implemented: true,
            example: "minio://bucket/prefix".to_string(),
        },
        SupportedSourceType {
            kind: "s3".to_string(),
            label: "S3 bucket".to_string(),
            implemented: true,
            example: "s3://bucket/prefix".to_string(),
        },
        SupportedSourceType {
            kind: "video".to_string(),
            label: "Video stream".to_string(),
            implemented: false,
            example: "video:///clips/demo.mp4".to_string(),
        },
        SupportedSourceType {
            kind: "camera".to_string(),
            label: "Camera".to_string(),
            implemented: false,
            example: "camera://front-door".to_string(),
        },
    ]
}

pub(crate) fn video_source_extensions() -> Vec<String> {
    [".mp4", ".mov", ".m4v", ".webm", ".mkv", ".avi"]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn source_status(
    diagnostics: &[SourceDiagnostic],
    capabilities: &SourceCapabilities,
) -> String {
    if !capabilities.enumerates_items {
        return "unsupported".to_string();
    }
    if diagnostics
        .iter()
        .any(|diagnostic| matches!(diagnostic.code, SourceDiagnosticCode::ParseError))
    {
        return "invalid".to_string();
    }
    if diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code,
            SourceDiagnosticCode::Unavailable
                | SourceDiagnosticCode::CredentialsMissing
                | SourceDiagnosticCode::EnumerationFailed
        )
    }) {
        return "unavailable".to_string();
    }
    if diagnostics
        .iter()
        .any(|diagnostic| matches!(diagnostic.code, SourceDiagnosticCode::EmptySource))
    {
        return "empty".to_string();
    }
    if diagnostics.iter().any(|diagnostic| {
        matches!(
            diagnostic.code,
            SourceDiagnosticCode::ModelFeatureUnavailable
                | SourceDiagnosticCode::IndexingFailed
                | SourceDiagnosticCode::UnsupportedKind
        )
    }) {
        return "degraded".to_string();
    }
    "ready".to_string()
}

fn object_source_config_diagnostics(
    spec: &str,
    kind: &str,
    settings: &Settings,
) -> Vec<SourceDiagnostic> {
    let Ok(url) = url::Url::parse(spec) else {
        return vec![SourceDiagnostic::error(
            SourceDiagnosticCode::ParseError,
            format!("Invalid object-store source URI: {spec}"),
        )];
    };
    if url.host_str().filter(|bucket| !bucket.is_empty()).is_none() {
        return vec![SourceDiagnostic::error(
            SourceDiagnosticCode::ParseError,
            format!("Missing bucket in object-store source URI: {spec}"),
        )];
    }

    let endpoint = match kind {
        "minio" => settings
            .minio_endpoint
            .clone()
            .or_else(|| settings.s3_endpoint.clone()),
        "s3" => settings
            .s3_endpoint
            .clone()
            .or_else(|| settings.minio_endpoint.clone()),
        _ => None,
    };
    let access_key = match kind {
        "minio" => settings
            .minio_access_key
            .clone()
            .or_else(|| settings.s3_access_key_id.clone()),
        "s3" => settings
            .s3_access_key_id
            .clone()
            .or_else(|| settings.minio_access_key.clone()),
        _ => None,
    };
    let secret_key = match kind {
        "minio" => settings
            .minio_secret_key
            .clone()
            .or_else(|| settings.s3_secret_access_key.clone()),
        "s3" => settings
            .s3_secret_access_key
            .clone()
            .or_else(|| settings.minio_secret_key.clone()),
        _ => None,
    };

    if kind == "minio" && endpoint.is_none() {
        return vec![SourceDiagnostic::error(
            SourceDiagnosticCode::CredentialsMissing,
            "MINIO_ENDPOINT or S3_ENDPOINT is required for MinIO sources",
        )];
    }
    if endpoint.is_some() && (access_key.is_none() || secret_key.is_none()) {
        return vec![SourceDiagnostic::error(
            SourceDiagnosticCode::CredentialsMissing,
            format!(
                "{} object-store credentials are incomplete",
                kind.to_ascii_uppercase()
            ),
        )];
    }

    Vec::new()
}

fn local_source_path(spec: &str) -> std::path::PathBuf {
    match url::Url::parse(spec) {
        Ok(url) if url.scheme() == "file" => {
            url.to_file_path().unwrap_or_else(|_| url.path().into())
        }
        Ok(url) if url.scheme() == "local" => {
            let mut path = String::new();
            if let Some(host) = url.host_str() {
                path.push('/');
                path.push_str(host);
            }
            path.push_str(url.path());
            path.into()
        }
        _ => spec.into(),
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Settings;

    use super::{source_config_source, supported_source_types};

    #[test]
    fn source_config_source_rejects_unsupported_schemes() {
        let source = source_config_source(
            "ftp://example.test/archive".to_string(),
            &Settings::default(),
        );
        assert_eq!(source.kind, "ftp");
        assert_eq!(source.status, "unsupported");
        assert!(source
            .detail
            .as_deref()
            .is_some_and(|detail| { detail.contains("Unsupported media source kind") }));
    }

    #[test]
    fn source_types_include_expected_kinds() {
        let kinds: Vec<_> = supported_source_types()
            .into_iter()
            .map(|entry| entry.kind)
            .collect();
        assert!(kinds.contains(&"local".to_string()));
        assert!(kinds.contains(&"s3".to_string()));
    }
}
