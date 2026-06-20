use crate::config::Settings;

use super::contracts::{SourceConfigSource, SupportedSourceType};

pub(crate) fn source_config_source(spec: String, settings: &Settings) -> SourceConfigSource {
    let kind = source_kind(&spec);
    let (status, detail) = match kind.as_str() {
        "local" => {
            let path = local_source_path(&spec);
            if path.is_dir() {
                ("ready".to_string(), None)
            } else {
                (
                    "unavailable".to_string(),
                    Some(format!("Directory does not exist: {}", path.display())),
                )
            }
        }
        "minio" | "s3" => object_source_config_status(&spec, &kind, settings),
        "video" => (
            "not_implemented".to_string(),
            Some(
                "Video source specs are not implemented; local folders can include video files"
                    .to_string(),
            ),
        ),
        "camera" => (
            "not_implemented".to_string(),
            Some("Camera sources are not implemented in the native Rust service yet".to_string()),
        ),
        _ => (
            "unsupported".to_string(),
            Some(format!("Unsupported media source: {spec}")),
        ),
    };

    SourceConfigSource {
        spec,
        kind,
        status,
        detail,
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

pub(crate) fn source_kind(spec: &str) -> String {
    if let Some((scheme, _)) = spec.split_once(':') {
        if scheme.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
        }) {
            return match scheme {
                "file" | "local" => "local".to_string(),
                other => other.to_string(),
            };
        }
    }
    "local".to_string()
}

fn object_source_config_status(
    spec: &str,
    kind: &str,
    settings: &Settings,
) -> (String, Option<String>) {
    let Ok(url) = url::Url::parse(spec) else {
        return (
            "unavailable".to_string(),
            Some(format!("Invalid object-store source URI: {spec}")),
        );
    };
    if url.host_str().filter(|bucket| !bucket.is_empty()).is_none() {
        return (
            "unavailable".to_string(),
            Some(format!("Missing bucket in object-store source URI: {spec}")),
        );
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
        return (
            "unavailable".to_string(),
            Some("MINIO_ENDPOINT or S3_ENDPOINT is required for MinIO sources".to_string()),
        );
    }
    if endpoint.is_some() && (access_key.is_none() || secret_key.is_none()) {
        return (
            "unavailable".to_string(),
            Some(format!(
                "{} object-store credentials are incomplete",
                kind.to_ascii_uppercase()
            )),
        );
    }

    ("ready".to_string(), None)
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
            .is_some_and(|detail| detail.contains("Unsupported media source")));
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
