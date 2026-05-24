use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use futures_util::TryStreamExt;
use object_store::aws::AmazonS3Builder;
use object_store::path::Path as ObjectPath;
use object_store::{ObjectStore, ObjectStoreExt};
use sha2::{Digest, Sha256};
use url::Url;

use crate::config::{Settings, SourceSettings};
use crate::workers::media::audio::{decode_audio, is_audio_extension};
use crate::workers::media::image_io::{iter_image_paths, load_media, relative_path};
use crate::workers::media::media::DecodedMedia;
use crate::workers::media::pdf::is_pdf_extension;
use crate::workers::media::video::is_video_extension;

#[derive(Clone, Debug)]
pub struct SourceImage {
    pub source_type: String,
    pub source_uri: String,
    #[allow(dead_code)]
    pub item_uri: String,
    pub id_base: String,
    pub display_path: String,
    pub relative_path: String,
    pub filename: String,
    pub size_bytes: u64,
    pub modified_at: f64,
    loader: SourceLoader,
}

impl SourceImage {
    #[cfg(test)]
    pub(crate) fn test_local_image(path: &str, size_bytes: u64, modified_at: f64) -> Self {
        let path = PathBuf::from(path);
        Self {
            source_type: "local".to_string(),
            source_uri: "/images".to_string(),
            item_uri: path.to_string_lossy().to_string(),
            id_base: path.to_string_lossy().to_string(),
            display_path: path.to_string_lossy().to_string(),
            relative_path: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string(),
            filename: path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string(),
            size_bytes,
            modified_at,
            loader: SourceLoader::LocalImage(path),
        }
    }

    pub fn is_video(&self) -> bool {
        matches!(self.loader, SourceLoader::LocalVideo(_))
            || matches!(
                &self.loader,
                SourceLoader::ObjectStoreObject(object)
                    if matches!(object.kind, ObjectSourceKind::Video)
            )
    }

    pub fn is_audio(&self) -> bool {
        matches!(self.loader, SourceLoader::LocalAudio(_))
            || matches!(
                &self.loader,
                SourceLoader::ObjectStoreObject(object)
                    if matches!(object.kind, ObjectSourceKind::Audio)
            )
    }

    pub fn is_pdf(&self) -> bool {
        matches!(self.loader, SourceLoader::LocalPdf(_))
            || matches!(
                &self.loader,
                SourceLoader::ObjectStoreObject(object)
                    if matches!(object.kind, ObjectSourceKind::Pdf)
            )
    }

    pub fn local_path(&self) -> Option<&PathBuf> {
        match &self.loader {
            SourceLoader::LocalImage(path)
            | SourceLoader::LocalVideo(path)
            | SourceLoader::LocalAudio(path)
            | SourceLoader::LocalPdf(path) => Some(path),
            SourceLoader::ObjectStoreObject(_) | SourceLoader::Unavailable(_) => None,
        }
    }

    pub async fn with_local_media_path<T>(
        &self,
        settings: &Settings,
        read: impl FnOnce(&Path) -> Result<T, String>,
    ) -> Result<T, String> {
        match &self.loader {
            SourceLoader::LocalImage(path)
            | SourceLoader::LocalVideo(path)
            | SourceLoader::LocalAudio(path)
            | SourceLoader::LocalPdf(path) => read(path),
            SourceLoader::ObjectStoreObject(object) => {
                let path = self.download_object(settings, object).await?;
                let result = read(&path);
                let _ = fs::remove_file(&path);
                if let Some(parent) = path.parent() {
                    let _ = fs::remove_dir(parent);
                }
                result
            }
            SourceLoader::Unavailable(error) => Err(error.clone()),
        }
    }

    pub async fn load_media(&self, settings: &Settings) -> Result<DecodedMedia, String> {
        match &self.loader {
            SourceLoader::LocalImage(path) => load_media(path, settings),
            SourceLoader::LocalVideo(_) => Err("Video files expand into scene media".to_string()),
            SourceLoader::LocalAudio(path) => decode_audio(path, settings),
            SourceLoader::LocalPdf(_) => Err("PDF files expand into page media".to_string()),
            SourceLoader::ObjectStoreObject(_) => {
                self.with_local_media_path(settings, |path| {
                    if self.is_audio() {
                        decode_audio(path, settings)
                    } else {
                        load_media(path, settings)
                    }
                })
                .await
            }
            SourceLoader::Unavailable(error) => Err(error.clone()),
        }
    }

    async fn download_object(
        &self,
        settings: &Settings,
        object: &ObjectStoreObjectRef,
    ) -> Result<PathBuf, String> {
        let store = object_store_for(&object.scheme, &object.bucket, &settings.source_settings())?;
        let bytes = store
            .get(&ObjectPath::from(object.key.as_str()))
            .await
            .map_err(|error| format!("{}: {error}", self.item_uri))?
            .bytes()
            .await
            .map_err(|error| format!("{}: {error}", self.item_uri))?;
        let digest = Sha256::digest(self.item_uri.as_bytes());
        let cache_dir = settings
            .upload_dir
            .join("source-cache")
            .join(format!("{digest:x}"));
        fs::create_dir_all(&cache_dir).map_err(|error| {
            format!(
                "Could not create object source cache {}: {error}",
                cache_dir.display()
            )
        })?;
        let filename = self.filename.trim();
        let filename = if filename.is_empty() {
            "object"
        } else {
            filename
        };
        let final_path = cache_dir.join(filename);
        let part_path = final_path.with_extension(format!(
            "{}.part",
            final_path
                .extension()
                .and_then(|extension| extension.to_str())
                .unwrap_or("download")
        ));
        fs::write(&part_path, &bytes)
            .map_err(|error| format!("Could not write {}: {error}", part_path.display()))?;
        fs::rename(&part_path, &final_path).map_err(|error| {
            format!(
                "Could not move {} to {}: {error}",
                part_path.display(),
                final_path.display()
            )
        })?;
        Ok(final_path)
    }
}

#[derive(Clone, Debug)]
enum SourceLoader {
    LocalImage(PathBuf),
    LocalVideo(PathBuf),
    LocalAudio(PathBuf),
    LocalPdf(PathBuf),
    ObjectStoreObject(ObjectStoreObjectRef),
    Unavailable(String),
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug)]
pub enum ImageSource {
    Local(LocalFolderSource),
    ObjectStore(ObjectStoreSource),
    Unavailable(UnavailableSource),
}

impl ImageSource {
    pub fn uri(&self) -> String {
        match self {
            Self::Local(source) => source.uri(),
            Self::ObjectStore(source) => source.uri.clone(),
            Self::Unavailable(source) => source.uri.clone(),
        }
    }

    pub async fn iter_images(&self) -> Result<Vec<SourceImage>, SourceUnavailable> {
        match self {
            Self::Local(source) => source.iter_images(),
            Self::ObjectStore(source) => source.iter_images().await,
            Self::Unavailable(source) => Err(SourceUnavailable(source.error.clone())),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SourceUnavailable(pub String);

#[derive(Clone, Debug)]
pub struct UnavailableSource {
    uri: String,
    error: String,
}

#[derive(Clone, Debug)]
pub struct LocalFolderSource {
    root: PathBuf,
    extensions: BTreeSet<String>,
}

#[derive(Clone, Debug)]
pub struct ObjectStoreSource {
    scheme: String,
    bucket: String,
    prefix: String,
    uri: String,
    settings: SourceSettings,
}

#[derive(Clone, Debug)]
pub struct ObjectStoreObjectRef {
    scheme: String,
    bucket: String,
    key: String,
    kind: ObjectSourceKind,
}

#[derive(Clone, Copy, Debug)]
enum ObjectSourceKind {
    Audio,
    Image,
    Pdf,
    Video,
}

impl LocalFolderSource {
    pub fn new(
        root: PathBuf,
        image_extensions: BTreeSet<String>,
        audio_extensions: BTreeSet<String>,
        pdf_extensions: BTreeSet<String>,
    ) -> Self {
        let mut extensions = image_extensions;
        extensions.extend(video_extensions());
        extensions.extend(audio_extensions);
        extensions.extend(pdf_extensions);
        Self { root, extensions }
    }

    pub fn uri(&self) -> String {
        self.root.to_string_lossy().to_string()
    }

    pub fn iter_images(&self) -> Result<Vec<SourceImage>, SourceUnavailable> {
        if !self.root.exists() {
            return Err(SourceUnavailable(format!(
                "Source directory does not exist: {}",
                self.root.display()
            )));
        }

        let mut images = Vec::new();
        for path in iter_image_paths(&self.root, &self.extensions) {
            let stat = path
                .metadata()
                .map_err(|error| SourceUnavailable(format!("{}: {error}", path.display())))?;
            let resolved = path.canonicalize().unwrap_or_else(|_| path.clone());
            let modified_at = stat
                .modified()
                .ok()
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs_f64())
                .unwrap_or(0.0);
            let is_video = path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| is_video_extension(&format!(".{extension}")))
                .unwrap_or(false);
            let is_audio = path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| is_audio_extension(&format!(".{extension}")))
                .unwrap_or(false);
            let is_pdf = path
                .extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| is_pdf_extension(&format!(".{extension}")))
                .unwrap_or(false);
            images.push(SourceImage {
                source_type: "local".to_string(),
                source_uri: self.uri(),
                item_uri: resolved.to_string_lossy().to_string(),
                id_base: resolved.to_string_lossy().to_string(),
                display_path: resolved.to_string_lossy().to_string(),
                relative_path: relative_path(&path, &self.root),
                filename: path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
                    .to_string(),
                size_bytes: stat.len(),
                modified_at,
                loader: if is_video {
                    SourceLoader::LocalVideo(path)
                } else if is_audio {
                    SourceLoader::LocalAudio(path)
                } else if is_pdf {
                    SourceLoader::LocalPdf(path)
                } else {
                    SourceLoader::LocalImage(path)
                },
            });
        }
        Ok(images)
    }
}

impl ObjectStoreSource {
    fn from_url(url: &Url, settings: &Settings) -> Result<Self, String> {
        let scheme = url.scheme().to_string();
        let bucket = url
            .host_str()
            .filter(|bucket| !bucket.trim().is_empty())
            .ok_or_else(|| format!("Missing bucket in {url}"))?
            .to_string();
        let prefix = normalized_object_prefix(url.path());
        let uri = match scheme.as_str() {
            "minio" => minio_uri(url),
            "s3" => s3_uri(url),
            _ => object_store_uri(&scheme, &bucket, &prefix),
        };
        Ok(Self {
            scheme,
            bucket,
            prefix,
            uri,
            settings: settings.source_settings(),
        })
    }

    async fn iter_images(&self) -> Result<Vec<SourceImage>, SourceUnavailable> {
        let store = object_store_for(&self.scheme, &self.bucket, &self.settings)
            .map_err(|error| SourceUnavailable(format!("{}: {error}", self.uri)))?;
        let prefix = if self.prefix.is_empty() {
            None
        } else {
            Some(ObjectPath::from(self.prefix.as_str()))
        };
        let objects = store
            .list(prefix.as_ref())
            .try_collect::<Vec<_>>()
            .await
            .map_err(|error| SourceUnavailable(format!("{}: {error}", self.uri)))?;
        let mut images = Vec::new();
        for object in objects {
            let key = object.location.to_string();
            if key.ends_with('/') {
                continue;
            }
            let Some(filename) = key.rsplit('/').next().filter(|part| !part.is_empty()) else {
                continue;
            };
            let extension = filename
                .rsplit_once('.')
                .map(|(_, extension)| format!(".{}", extension.to_ascii_lowercase()));
            let Some(kind) = extension
                .as_ref()
                .and_then(|extension| object_source_kind(extension, &self.settings))
            else {
                continue;
            };
            let item_uri = object_store_uri(&self.scheme, &self.bucket, &key);
            images.push(SourceImage {
                source_type: self.scheme.clone(),
                source_uri: self.uri.clone(),
                item_uri: item_uri.clone(),
                id_base: item_uri.clone(),
                display_path: item_uri,
                relative_path: object_relative_path(&key, &self.prefix),
                filename: filename.to_string(),
                size_bytes: object.size,
                modified_at: object
                    .last_modified
                    .timestamp_nanos_opt()
                    .map(|nanos| nanos as f64 / 1_000_000_000.0)
                    .unwrap_or_else(|| object.last_modified.timestamp() as f64),
                loader: SourceLoader::ObjectStoreObject(ObjectStoreObjectRef {
                    scheme: self.scheme.clone(),
                    bucket: self.bucket.clone(),
                    key,
                    kind,
                }),
            });
        }
        Ok(images)
    }
}

fn object_source_kind(extension: &str, settings: &SourceSettings) -> Option<ObjectSourceKind> {
    if is_video_extension(extension) {
        Some(ObjectSourceKind::Video)
    } else if settings.audio_extensions.contains(extension) {
        Some(ObjectSourceKind::Audio)
    } else if settings.pdf_extensions.contains(extension) {
        Some(ObjectSourceKind::Pdf)
    } else if settings.image_extensions.contains(extension) {
        Some(ObjectSourceKind::Image)
    } else {
        None
    }
}

pub fn build_image_sources(settings: &Settings) -> Vec<ImageSource> {
    settings
        .source_specs()
        .into_iter()
        .map(|spec| source_from_spec(&spec, settings))
        .collect()
}

fn source_from_spec(spec: &str, settings: &Settings) -> ImageSource {
    match Url::parse(spec) {
        Ok(url) => match url.scheme() {
            "" | "file" | "local" => ImageSource::Local(LocalFolderSource::new(
                path_from_url(&url),
                settings.image_extensions.clone(),
                settings.audio_extensions.clone(),
                settings.pdf_extensions.clone(),
            )),
            "minio" | "s3" => match ObjectStoreSource::from_url(&url, settings) {
                Ok(source) => ImageSource::ObjectStore(source),
                Err(error) => ImageSource::Unavailable(UnavailableSource {
                    uri: spec.to_string(),
                    error,
                }),
            },
            "video" => ImageSource::Unavailable(UnavailableSource {
                uri: spec.to_string(),
                error: "Video sources are not implemented in the native Rust service yet"
                    .to_string(),
            }),
            "camera" => ImageSource::Unavailable(UnavailableSource {
                uri: spec.to_string(),
                error: "Camera sources are not implemented in the native Rust service yet"
                    .to_string(),
            }),
            _ => ImageSource::Unavailable(UnavailableSource {
                uri: spec.to_string(),
                error: format!("Unsupported image source: {spec}"),
            }),
        },
        Err(_) => ImageSource::Local(LocalFolderSource::new(
            PathBuf::from(spec),
            settings.image_extensions.clone(),
            settings.audio_extensions.clone(),
            settings.pdf_extensions.clone(),
        )),
    }
}

fn path_from_url(url: &Url) -> PathBuf {
    if url.scheme() == "file" {
        return url
            .to_file_path()
            .unwrap_or_else(|_| PathBuf::from(url.path()));
    }
    let mut path = String::new();
    if let Some(host) = url.host_str() {
        path.push('/');
        path.push_str(host);
    }
    path.push_str(url.path());
    PathBuf::from(if path.is_empty() { url.path() } else { &path })
}

fn minio_uri(url: &Url) -> String {
    let bucket = url.host_str().unwrap_or_default();
    object_store_uri("minio", bucket, &normalized_object_prefix(url.path()))
}

fn s3_uri(url: &Url) -> String {
    let bucket = url.host_str().unwrap_or_default();
    object_store_uri("s3", bucket, &normalized_object_prefix(url.path()))
}

fn object_store_uri(scheme: &str, bucket: &str, key_or_prefix: &str) -> String {
    if key_or_prefix.is_empty() {
        format!("{scheme}://{bucket}")
    } else {
        format!(
            "{scheme}://{bucket}/{}",
            key_or_prefix.trim_start_matches('/')
        )
    }
}

fn normalized_object_prefix(path: &str) -> String {
    path.trim_start_matches('/')
        .trim_end_matches('/')
        .to_string()
}

fn object_relative_path(key: &str, prefix: &str) -> String {
    let prefix = prefix.trim_matches('/');
    if prefix.is_empty() {
        return key.to_string();
    }
    key.strip_prefix(prefix)
        .map(|relative| relative.trim_start_matches('/').to_string())
        .filter(|relative| !relative.is_empty())
        .unwrap_or_else(|| key.to_string())
}

fn object_store_for(
    scheme: &str,
    bucket: &str,
    settings: &SourceSettings,
) -> Result<object_store::aws::AmazonS3, String> {
    if bucket.trim().is_empty() {
        return Err("object-store source is missing a bucket".to_string());
    }

    let endpoint = object_store_endpoint(scheme, settings);
    let access_key = object_store_access_key(scheme, settings);
    let secret_key = object_store_secret_key(scheme, settings);
    let region = if scheme == "s3" {
        settings.s3_region.clone()
    } else {
        settings
            .s3_region
            .clone()
            .if_empty_then("us-east-1".to_string())
    };
    let allow_http = if scheme == "minio" {
        !settings.minio_secure || settings.s3_allow_http
    } else {
        settings.s3_allow_http
    };

    let mut builder = AmazonS3Builder::from_env()
        .with_bucket_name(bucket)
        .with_region(region);
    if let Some(endpoint) = endpoint {
        builder = builder
            .with_endpoint(endpoint)
            .with_virtual_hosted_style_request(false);
    }
    if let Some(access_key) = access_key {
        builder = builder.with_access_key_id(access_key);
    }
    if let Some(secret_key) = secret_key {
        builder = builder.with_secret_access_key(secret_key);
    }
    if allow_http {
        builder = builder.with_allow_http(true);
    }
    builder.build().map_err(|error| error.to_string())
}

fn object_store_endpoint(scheme: &str, settings: &SourceSettings) -> Option<String> {
    let endpoint = match scheme {
        "minio" => settings
            .minio_endpoint
            .clone()
            .or_else(|| settings.s3_endpoint.clone()),
        "s3" => settings
            .s3_endpoint
            .clone()
            .or_else(|| settings.minio_endpoint.clone()),
        _ => None,
    }?;
    Some(normalized_endpoint(
        endpoint,
        if scheme == "minio" {
            settings.minio_secure
        } else {
            !settings.s3_allow_http
        },
    ))
}

fn normalized_endpoint(endpoint: String, secure: bool) -> String {
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        endpoint
    } else if secure {
        format!("https://{endpoint}")
    } else {
        format!("http://{endpoint}")
    }
}

fn object_store_access_key(scheme: &str, settings: &SourceSettings) -> Option<String> {
    match scheme {
        "minio" => settings
            .minio_access_key
            .clone()
            .or_else(|| settings.s3_access_key_id.clone()),
        "s3" => settings
            .s3_access_key_id
            .clone()
            .or_else(|| settings.minio_access_key.clone()),
        _ => None,
    }
}

fn object_store_secret_key(scheme: &str, settings: &SourceSettings) -> Option<String> {
    match scheme {
        "minio" => settings
            .minio_secret_key
            .clone()
            .or_else(|| settings.s3_secret_access_key.clone()),
        "s3" => settings
            .s3_secret_access_key
            .clone()
            .or_else(|| settings.minio_secret_key.clone()),
        _ => None,
    }
}

trait EmptyStringDefault {
    fn if_empty_then(self, default: String) -> String;
}

impl EmptyStringDefault for String {
    fn if_empty_then(self, default: String) -> String {
        if self.trim().is_empty() {
            default
        } else {
            self
        }
    }
}

fn video_extensions() -> BTreeSet<String> {
    [".mp4", ".mov", ".m4v", ".webm", ".mkv", ".avi"]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

#[allow(dead_code)]
fn unavailable_image(error: String) -> SourceLoader {
    SourceLoader::Unavailable(error)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use image::{ImageBuffer, Rgb};

    use super::{build_image_sources, minio_uri, object_relative_path, s3_uri, ImageSource};
    use crate::config::Settings;

    #[test]
    fn build_sources_defaults_to_source_image_dir() {
        let settings = Settings::default();
        let sources = build_image_sources(&settings);
        assert_eq!(sources.len(), 1);
        assert!(matches!(sources[0], ImageSource::Local(_)));
    }

    #[tokio::test]
    async fn local_folder_source_yields_metadata_and_loads_images() {
        let dir = tempfile_dir();
        let image_path = dir.join("sample.jpg");
        ImageBuffer::from_pixel(64, 48, Rgb([1_u8, 2_u8, 3_u8]))
            .save(&image_path)
            .unwrap();
        let settings = Settings {
            source_image_dir: dir.clone(),
            ..Settings::default()
        };
        let source = build_image_sources(&settings).remove(0);
        let items = source.iter_images().await.unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source_type, "local");
        assert_eq!(items[0].relative_path, "sample.jpg");
        let media = items[0].load_media(&settings).await.unwrap();
        assert_eq!((media.width, media.height), (64, 48));
    }

    #[tokio::test]
    async fn local_folder_source_yields_video_files() {
        let dir = tempfile_dir();
        fs::write(dir.join("clip.mp4"), b"not a real video").unwrap();
        let settings = Settings {
            source_image_dir: dir.clone(),
            ..Settings::default()
        };
        let source = build_image_sources(&settings).remove(0);
        let items = source.iter_images().await.unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].is_video());
        assert_eq!(items[0].relative_path, "clip.mp4");
    }

    #[tokio::test]
    async fn local_folder_source_yields_audio_files() {
        let dir = tempfile_dir();
        fs::write(dir.join("song.mp3"), b"not real audio").unwrap();
        let settings = Settings {
            source_image_dir: dir.clone(),
            ..Settings::default()
        };
        let source = build_image_sources(&settings).remove(0);
        let items = source.iter_images().await.unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].is_audio());
        assert_eq!(items[0].relative_path, "song.mp3");
    }

    #[tokio::test]
    async fn local_folder_source_yields_pdf_files() {
        let dir = tempfile_dir();
        fs::write(dir.join("paper.PDF"), b"%PDF-1.4\n").unwrap();
        let settings = Settings {
            source_image_dir: dir.clone(),
            ..Settings::default()
        };
        let source = build_image_sources(&settings).remove(0);
        let items = source.iter_images().await.unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].is_pdf());
        assert_eq!(items[0].relative_path, "paper.PDF");
    }

    #[tokio::test]
    async fn unsupported_sources_are_reported_without_panicking() {
        let settings = Settings {
            image_sources: vec![
                "minio://images/catalog".to_string(),
                "video:///demo.mp4".to_string(),
            ],
            ..Settings::default()
        };
        let sources = build_image_sources(&settings);
        assert!(matches!(sources[0], ImageSource::ObjectStore(_)));
        assert!(sources[0].iter_images().await.is_err());
        assert!(matches!(sources[1], ImageSource::Unavailable(_)));
    }

    #[test]
    fn object_store_uri_normalization_preserves_bucket_and_prefix() {
        let minio = url::Url::parse("minio://images/catalog/").unwrap();
        let s3 = url::Url::parse("s3://archive/family/2024").unwrap();
        assert_eq!(minio_uri(&minio), "minio://images/catalog");
        assert_eq!(s3_uri(&s3), "s3://archive/family/2024");
    }

    #[test]
    fn object_relative_paths_trim_configured_prefix() {
        assert_eq!(
            object_relative_path("family/2024/photo.jpg", "family/2024"),
            "photo.jpg"
        );
        assert_eq!(
            object_relative_path("other/photo.jpg", "family/2024"),
            "other/photo.jpg"
        );
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("image-sim-rust-source-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
