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
