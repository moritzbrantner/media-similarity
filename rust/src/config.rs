use std::collections::BTreeSet;
use std::env;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub source_image_dir: PathBuf,
    pub qdrant_url: String,
    pub qdrant_collection: String,
    pub clip_model_name: String,
    pub thumbnail_dir: PathBuf,
    pub upload_dir: PathBuf,
    pub image_extensions: BTreeSet<String>,
    pub image_sources: Vec<String>,
    pub minio_endpoint: Option<String>,
    pub minio_access_key: Option<String>,
    pub minio_secret_key: Option<String>,
    pub minio_secure: bool,
    pub video_frame_stride: u32,
    pub video_max_frames: Option<u32>,
    pub camera_frame_stride: u32,
    pub camera_max_frames: u32,
    pub default_search_limit: u32,
    pub duplicate_hash_distance: u32,
    pub max_upload_mb: u32,
    pub vector_size: usize,
    pub bind_addr: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            source_image_dir: PathBuf::from("/images"),
            qdrant_url: "http://qdrant:6333".to_string(),
            qdrant_collection: "image_similarity".to_string(),
            clip_model_name: "sentence-transformers/clip-ViT-B-32".to_string(),
            thumbnail_dir: PathBuf::from("data/thumbnails"),
            upload_dir: PathBuf::from("data/uploads"),
            image_extensions: parse_extensions(".jpg,.jpeg,.png,.webp,.bmp,.tif,.tiff")
                .expect("default extensions are valid"),
            image_sources: Vec::new(),
            minio_endpoint: None,
            minio_access_key: None,
            minio_secret_key: None,
            minio_secure: true,
            video_frame_stride: 30,
            video_max_frames: None,
            camera_frame_stride: 30,
            camera_max_frames: 100,
            default_search_limit: 12,
            duplicate_hash_distance: 8,
            max_upload_mb: 20,
            vector_size: 512,
            bind_addr: "0.0.0.0:8000".to_string(),
        }
    }
}

impl Settings {
    pub fn from_env() -> Result<Self, String> {
        dotenvy::dotenv().ok();
        let defaults = Self::default();
        Ok(Self {
            source_image_dir: path_var("SOURCE_IMAGE_DIR", defaults.source_image_dir),
            qdrant_url: string_var("QDRANT_URL", defaults.qdrant_url),
            qdrant_collection: string_var("QDRANT_COLLECTION", defaults.qdrant_collection),
            clip_model_name: string_var("CLIP_MODEL_NAME", defaults.clip_model_name),
            thumbnail_dir: path_var("THUMBNAIL_DIR", defaults.thumbnail_dir),
            upload_dir: path_var("UPLOAD_DIR", defaults.upload_dir),
            image_extensions: match env::var("IMAGE_EXTENSIONS") {
                Ok(value) => parse_extensions(&value)?,
                Err(_) => defaults.image_extensions,
            },
            image_sources: env::var("IMAGE_SOURCES")
                .ok()
                .map(|value| parse_image_sources(&value))
                .transpose()?
                .unwrap_or_default(),
            minio_endpoint: optional_string_var("MINIO_ENDPOINT"),
            minio_access_key: optional_string_var("MINIO_ACCESS_KEY"),
            minio_secret_key: optional_string_var("MINIO_SECRET_KEY"),
            minio_secure: bool_var("MINIO_SECURE", defaults.minio_secure),
            video_frame_stride: bounded_u32_var(
                "VIDEO_FRAME_STRIDE",
                defaults.video_frame_stride,
                1,
                u32::MAX,
            )?,
            video_max_frames: optional_bounded_u32_var("VIDEO_MAX_FRAMES", 1, u32::MAX)?,
            camera_frame_stride: bounded_u32_var(
                "CAMERA_FRAME_STRIDE",
                defaults.camera_frame_stride,
                1,
                u32::MAX,
            )?,
            camera_max_frames: bounded_u32_var(
                "CAMERA_MAX_FRAMES",
                defaults.camera_max_frames,
                1,
                u32::MAX,
            )?,
            default_search_limit: bounded_u32_var(
                "DEFAULT_SEARCH_LIMIT",
                defaults.default_search_limit,
                1,
                100,
            )?,
            duplicate_hash_distance: bounded_u32_var(
                "DUPLICATE_HASH_DISTANCE",
                defaults.duplicate_hash_distance,
                0,
                64,
            )?,
            max_upload_mb: bounded_u32_var("MAX_UPLOAD_MB", defaults.max_upload_mb, 1, 200)?,
            vector_size: bounded_usize_var("VECTOR_SIZE", defaults.vector_size, 1, usize::MAX)?,
            bind_addr: string_var("BIND_ADDR", defaults.bind_addr),
        })
    }

    pub fn source_specs(&self) -> Vec<String> {
        if self.image_sources.is_empty() {
            vec![self.source_image_dir.to_string_lossy().to_string()]
        } else {
            self.image_sources.clone()
        }
    }
}

pub fn parse_extensions(value: &str) -> Result<BTreeSet<String>, String> {
    let extensions = value
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            let lower = part.to_ascii_lowercase();
            if lower.starts_with('.') {
                lower
            } else {
                format!(".{lower}")
            }
        })
        .collect::<BTreeSet<_>>();
    if extensions.is_empty() {
        Err("At least one image extension is required".to_string())
    } else {
        Ok(extensions)
    }
}

pub fn parse_image_sources(value: &str) -> Result<Vec<String>, String> {
    let stripped = value.trim();
    if stripped.is_empty() {
        return Ok(Vec::new());
    }
    if stripped.starts_with('[') {
        let parsed: Vec<String> = serde_json::from_str(stripped)
            .map_err(|error| format!("IMAGE_SOURCES must be a JSON string array: {error}"))?;
        return Ok(parsed
            .into_iter()
            .map(|part| part.trim().to_string())
            .filter(|part| !part.is_empty())
            .collect());
    }
    for separator in ['\n', ';', ','] {
        if stripped.contains(separator) {
            return Ok(stripped
                .split(separator)
                .map(str::trim)
                .filter(|part| !part.is_empty())
                .map(ToOwned::to_owned)
                .collect());
        }
    }
    Ok(vec![stripped.to_string()])
}

fn string_var(name: &str, default: String) -> String {
    optional_string_var(name).unwrap_or(default)
}

fn optional_string_var(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn path_var(name: &str, default: PathBuf) -> PathBuf {
    optional_string_var(name)
        .map(PathBuf::from)
        .unwrap_or(default)
}

fn bool_var(name: &str, default: bool) -> bool {
    optional_string_var(name)
        .map(|value| {
            matches!(
                value.to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(default)
}

fn bounded_u32_var(name: &str, default: u32, min: u32, max: u32) -> Result<u32, String> {
    match optional_string_var(name) {
        Some(value) => {
            let parsed = value
                .parse::<u32>()
                .map_err(|_| format!("{name} must be an integer"))?;
            if parsed < min || parsed > max {
                Err(format!("{name} must be between {min} and {max}"))
            } else {
                Ok(parsed)
            }
        }
        None => Ok(default),
    }
}

fn optional_bounded_u32_var(name: &str, min: u32, max: u32) -> Result<Option<u32>, String> {
    match optional_string_var(name) {
        Some(value) => {
            let parsed = value
                .parse::<u32>()
                .map_err(|_| format!("{name} must be an integer"))?;
            if parsed < min || parsed > max {
                Err(format!("{name} must be between {min} and {max}"))
            } else {
                Ok(Some(parsed))
            }
        }
        None => Ok(None),
    }
}

fn bounded_usize_var(name: &str, default: usize, min: usize, max: usize) -> Result<usize, String> {
    match optional_string_var(name) {
        Some(value) => {
            let parsed = value
                .parse::<usize>()
                .map_err(|_| format!("{name} must be an integer"))?;
            if parsed < min || parsed > max {
                Err(format!("{name} must be between {min} and {max}"))
            } else {
                Ok(parsed)
            }
        }
        None => Ok(default),
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_extensions, parse_image_sources};

    #[test]
    fn extensions_are_normalized() {
        let parsed = parse_extensions("jpg, .PNG, webp").unwrap();
        assert_eq!(
            parsed.into_iter().collect::<Vec<_>>(),
            vec![".jpg", ".png", ".webp"]
        );
    }

    #[test]
    fn image_sources_accept_delimited_strings_and_json() {
        assert_eq!(
            parse_image_sources("local:///images; minio://bucket/prefix").unwrap(),
            vec!["local:///images", "minio://bucket/prefix"]
        );
        assert_eq!(
            parse_image_sources(r#"["/images", "video:///clips/demo.mp4"]"#).unwrap(),
            vec!["/images", "video:///clips/demo.mp4"]
        );
    }

    #[test]
    fn empty_extensions_are_rejected() {
        assert!(parse_extensions(" , ").is_err());
    }
}
