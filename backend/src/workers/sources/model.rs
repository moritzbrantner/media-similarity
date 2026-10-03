use std::path::{Path as SourcePath, PathBuf as SourcePathBuf};

use serde::Serialize;
use sha2::{Digest as SourceDigest, Sha256 as SourceSha256};
use url::Url as SourceUrl;

#[derive(Clone, Debug, Serialize)]
pub struct MediaSource {
    pub id: String,
    pub kind: SourceKind,
    pub original_spec: String,
    pub normalized_uri: String,
    pub display_name: Option<String>,
    pub capabilities: SourceCapabilities,
    pub diagnostics: Vec<SourceDiagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Local,
    S3,
    Minio,
    Unsupported(String),
}

impl SourceKind {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Local => "local",
            Self::S3 => "s3",
            Self::Minio => "minio",
            Self::Unsupported(kind) => kind.as_str(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceCapabilities {
    pub enumerates_items: bool,
    pub supports_images: bool,
    pub supports_gifs: bool,
    pub supports_video_files: bool,
    pub supports_audio_files: bool,
    pub supports_pdfs: bool,
    pub requires_credentials: bool,
}

impl SourceCapabilities {
    fn implemented(requires_credentials: bool) -> Self {
        Self {
            enumerates_items: true,
            supports_images: true,
            supports_gifs: true,
            supports_video_files: true,
            supports_audio_files: true,
            supports_pdfs: true,
            requires_credentials,
        }
    }

    fn unsupported(requires_credentials: bool) -> Self {
        Self {
            enumerates_items: false,
            supports_images: false,
            supports_gifs: false,
            supports_video_files: false,
            supports_audio_files: false,
            supports_pdfs: false,
            requires_credentials,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct SourceDiagnostic {
    pub code: SourceDiagnosticCode,
    pub severity: SourceDiagnosticSeverity,
    pub message: String,
}

impl SourceDiagnostic {
    pub fn error(code: SourceDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: SourceDiagnosticSeverity::Error,
            message: message.into(),
        }
    }

    pub fn warning(code: SourceDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: SourceDiagnosticSeverity::Warning,
            message: message.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceDiagnosticCode {
    ParseError,
    UnsupportedKind,
    Unavailable,
    CredentialsMissing,
    EnumerationFailed,
    EmptySource,
    ModelFeatureUnavailable,
    IndexingFailed,
}

pub fn parse_media_source_spec(spec: &str) -> MediaSource {
    let original_spec = spec.trim().to_string();
    let (kind, normalized_uri, display_name, mut diagnostics) =
        match SourceUrl::parse(&original_spec) {
            Ok(url) => match url.scheme() {
                "file" | "local" => {
                    let path = local_path_from_url(&url);
                    let normalized_uri = normalize_local_path(&path);
                    (
                        SourceKind::Local,
                        normalized_uri,
                        path.file_name()
                            .and_then(|name| name.to_str())
                            .map(ToOwned::to_owned),
                        Vec::new(),
                    )
                }
                "s3" => object_source_from_url(SourceKind::S3, &url),
                "minio" => object_source_from_url(SourceKind::Minio, &url),
                "video" | "camera" => {
                    let scheme = url.scheme().to_string();
                    (
                        SourceKind::Unsupported(scheme.clone()),
                        original_spec.clone(),
                        Some(scheme.clone()),
                        vec![SourceDiagnostic::warning(
                            SourceDiagnosticCode::UnsupportedKind,
                            format!("{scheme} sources are not implemented yet"),
                        )],
                    )
                }
                other => (
                    SourceKind::Unsupported(other.to_string()),
                    original_spec.clone(),
                    Some(other.to_string()),
                    vec![SourceDiagnostic::error(
                        SourceDiagnosticCode::UnsupportedKind,
                        format!("Unsupported media source kind `{other}`"),
                    )],
                ),
            },
            Err(_) => {
                let path = SourcePathBuf::from(&original_spec);
                (
                    SourceKind::Local,
                    normalize_local_path(&path),
                    path.file_name()
                        .and_then(|name| name.to_str())
                        .map(ToOwned::to_owned),
                    Vec::new(),
                )
            }
        };

    if original_spec.is_empty() {
        diagnostics.push(SourceDiagnostic::error(
            SourceDiagnosticCode::ParseError,
            "Source spec must not be empty",
        ));
    }

    let id = source_id(&kind, &normalized_uri);
    MediaSource {
        id,
        kind: kind.clone(),
        original_spec,
        normalized_uri,
        display_name,
        capabilities: capabilities_for_kind(&kind),
        diagnostics,
    }
}

pub fn source_id(kind: &SourceKind, normalized_uri: &str) -> String {
    let mut hasher = SourceSha256::new();
    hasher.update(kind.as_str().as_bytes());
    hasher.update(b":");
    hasher.update(normalized_uri.as_bytes());
    let digest = hasher.finalize();
    format!("src_v1_{}", hex_prefix(&digest, 8))
}

pub fn local_path_from_url(url: &SourceUrl) -> SourcePathBuf {
    if url.scheme() == "file" {
        return url
            .to_file_path()
            .unwrap_or_else(|_| SourcePathBuf::from(url.path()));
    }
    let mut path = String::new();
    if let Some(host) = url.host_str() {
        path.push('/');
        path.push_str(host);
    }
    path.push_str(url.path());
    SourcePathBuf::from(if path.is_empty() { url.path() } else { &path })
}

pub fn normalized_object_uri(scheme: &str, bucket: &str, path: &str) -> String {
    let prefix = normalized_source_object_prefix(path);
    if prefix.is_empty() {
        format!("{scheme}://{bucket}")
    } else {
        format!("{scheme}://{bucket}/{prefix}")
    }
}

pub fn normalized_source_object_prefix(path: &str) -> String {
    path.trim_start_matches('/')
        .trim_end_matches('/')
        .to_string()
}

fn object_source_from_url(
    kind: SourceKind,
    url: &SourceUrl,
) -> (SourceKind, String, Option<String>, Vec<SourceDiagnostic>) {
    let scheme = kind.as_str().to_string();
    let Some(bucket) = url.host_str().filter(|bucket| !bucket.trim().is_empty()) else {
        return (
            kind,
            url.to_string(),
            None,
            vec![SourceDiagnostic::error(
                SourceDiagnosticCode::ParseError,
                format!("Missing bucket in object-store source URI: {url}"),
            )],
        );
    };
    let normalized_uri = normalized_object_uri(&scheme, bucket, url.path());
    (kind, normalized_uri, Some(bucket.to_string()), Vec::new())
}

fn normalize_local_path(path: &SourcePath) -> String {
    path.canonicalize()
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

fn capabilities_for_kind(kind: &SourceKind) -> SourceCapabilities {
    match kind {
        SourceKind::Local => SourceCapabilities::implemented(false),
        SourceKind::S3 | SourceKind::Minio => SourceCapabilities::implemented(true),
        SourceKind::Unsupported(kind) => {
            SourceCapabilities::unsupported(matches!(kind.as_str(), "s3" | "minio"))
        }
    }
}

fn hex_prefix(bytes: &[u8], count: usize) -> String {
    bytes
        .iter()
        .take(count)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[cfg(test)]
mod model_tests {
    use super::{parse_media_source_spec, SourceDiagnosticCode};

    #[test]
    fn parses_and_normalizes_source_specs() {
        let local = parse_media_source_spec("/media/pictures");
        assert_eq!(local.kind.as_str(), "local");
        assert_eq!(local.normalized_uri, "/media/pictures");
        assert!(local.id.starts_with("src_v1_"));

        let file = parse_media_source_spec("file:///media/pictures");
        assert_eq!(file.kind.as_str(), "local");
        assert_eq!(file.normalized_uri, "/media/pictures");
        assert_eq!(file.id, local.id);

        let local_uri = parse_media_source_spec("local:///media/pictures");
        assert_eq!(local_uri.normalized_uri, "/media/pictures");
        assert_eq!(local_uri.id, local.id);

        let s3 = parse_media_source_spec("s3://bucket/prefix/");
        assert_eq!(s3.kind.as_str(), "s3");
        assert_eq!(s3.normalized_uri, "s3://bucket/prefix");

        let minio = parse_media_source_spec("minio://bucket//prefix/");
        assert_eq!(minio.kind.as_str(), "minio");
        assert_eq!(minio.normalized_uri, "minio://bucket/prefix");
    }

    #[test]
    fn source_id_changes_when_normalized_uri_changes() {
        let first = parse_media_source_spec("s3://bucket/a");
        let second = parse_media_source_spec("s3://bucket/b");
        assert_ne!(first.id, second.id);
    }

    #[test]
    fn reports_missing_bucket_and_unsupported_kinds() {
        let missing_bucket = parse_media_source_spec("s3:///prefix");
        assert!(missing_bucket
            .diagnostics
            .iter()
            .any(|diagnostic| matches!(diagnostic.code, SourceDiagnosticCode::ParseError)));

        let unsupported = parse_media_source_spec("ftp://example.test/archive");
        assert_eq!(unsupported.kind.as_str(), "ftp");
        assert!(unsupported
            .diagnostics
            .iter()
            .any(|diagnostic| matches!(diagnostic.code, SourceDiagnosticCode::UnsupportedKind)));
    }
}
