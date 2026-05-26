impl ImageSource {
    pub fn uri(&self) -> String {
        match self {
            Self::Local(source) => source.uri(),
            Self::ObjectStore(source) => source.uri.clone(),
            Self::Unavailable(source) => source.uri.clone(),
        }
    }

    pub fn local_root(&self) -> Option<&Path> {
        match self {
            Self::Local(source) => Some(source.root()),
            Self::ObjectStore(_) | Self::Unavailable(_) => None,
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

    pub fn root(&self) -> &Path {
        &self.root
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
