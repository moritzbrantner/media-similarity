use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use url::Url;

use crate::config::Settings;
use crate::workers::media::audio::{decode_audio, is_audio_extension};
use crate::workers::media::image_io::{iter_image_paths, load_media, relative_path};
use crate::workers::media::media::DecodedMedia;
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
    pub fn is_video(&self) -> bool {
        matches!(self.loader, SourceLoader::LocalVideo(_))
    }

    pub fn is_audio(&self) -> bool {
        matches!(self.loader, SourceLoader::LocalAudio(_))
    }

    pub fn local_path(&self) -> Option<&PathBuf> {
        match &self.loader {
            SourceLoader::LocalImage(path)
            | SourceLoader::LocalVideo(path)
            | SourceLoader::LocalAudio(path) => Some(path),
            SourceLoader::Unavailable(_) => None,
        }
    }

    pub fn load_media(&self, settings: &Settings) -> Result<DecodedMedia, String> {
        match &self.loader {
            SourceLoader::LocalImage(path) => load_media(path, settings),
            SourceLoader::LocalVideo(_) => Err("Video files expand into scene media".to_string()),
            SourceLoader::LocalAudio(path) => decode_audio(path, settings),
            SourceLoader::Unavailable(error) => Err(error.clone()),
        }
    }
}

#[derive(Clone, Debug)]
enum SourceLoader {
    LocalImage(PathBuf),
    LocalVideo(PathBuf),
    LocalAudio(PathBuf),
    Unavailable(String),
}

#[derive(Clone, Debug)]
pub enum ImageSource {
    Local(LocalFolderSource),
    Unavailable(UnavailableSource),
}

impl ImageSource {
    pub fn uri(&self) -> String {
        match self {
            Self::Local(source) => source.uri(),
            Self::Unavailable(source) => source.uri.clone(),
        }
    }

    pub fn iter_images(&self) -> Result<Vec<SourceImage>, SourceUnavailable> {
        match self {
            Self::Local(source) => source.iter_images(),
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

impl LocalFolderSource {
    pub fn new(
        root: PathBuf,
        image_extensions: BTreeSet<String>,
        audio_extensions: BTreeSet<String>,
    ) -> Self {
        let mut extensions = image_extensions;
        extensions.extend(video_extensions());
        extensions.extend(audio_extensions);
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
                } else {
                    SourceLoader::LocalImage(path)
                },
            });
        }
        Ok(images)
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
            )),
            "minio" => ImageSource::Unavailable(UnavailableSource {
                uri: minio_uri(&url),
                error: "MinIO sources are not implemented in the native Rust service yet"
                    .to_string(),
            }),
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
    let prefix = url.path().trim_start_matches('/');
    if prefix.is_empty() {
        format!("minio://{bucket}")
    } else {
        format!("minio://{bucket}/{prefix}")
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

    use super::{build_image_sources, ImageSource};
    use crate::config::Settings;

    #[test]
    fn build_sources_defaults_to_source_image_dir() {
        let settings = Settings::default();
        let sources = build_image_sources(&settings);
        assert_eq!(sources.len(), 1);
        assert!(matches!(sources[0], ImageSource::Local(_)));
    }

    #[test]
    fn local_folder_source_yields_metadata_and_loads_images() {
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
        let items = source.iter_images().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].source_type, "local");
        assert_eq!(items[0].relative_path, "sample.jpg");
        let media = items[0].load_media(&settings).unwrap();
        assert_eq!((media.width, media.height), (64, 48));
    }

    #[test]
    fn local_folder_source_yields_video_files() {
        let dir = tempfile_dir();
        fs::write(dir.join("clip.mp4"), b"not a real video").unwrap();
        let settings = Settings {
            source_image_dir: dir.clone(),
            ..Settings::default()
        };
        let source = build_image_sources(&settings).remove(0);
        let items = source.iter_images().unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].is_video());
        assert_eq!(items[0].relative_path, "clip.mp4");
    }

    #[test]
    fn local_folder_source_yields_audio_files() {
        let dir = tempfile_dir();
        fs::write(dir.join("song.mp3"), b"not real audio").unwrap();
        let settings = Settings {
            source_image_dir: dir.clone(),
            ..Settings::default()
        };
        let source = build_image_sources(&settings).remove(0);
        let items = source.iter_images().unwrap();
        assert_eq!(items.len(), 1);
        assert!(items[0].is_audio());
        assert_eq!(items[0].relative_path, "song.mp3");
    }

    #[test]
    fn unsupported_sources_are_reported_without_panicking() {
        let settings = Settings {
            image_sources: vec![
                "minio://images/catalog".to_string(),
                "video:///demo.mp4".to_string(),
            ],
            ..Settings::default()
        };
        let sources = build_image_sources(&settings);
        assert!(matches!(sources[0], ImageSource::Unavailable(_)));
        assert!(sources[0].iter_images().is_err());
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("image-sim-rust-source-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
