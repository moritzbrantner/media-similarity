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

    pub(crate) fn iter_local_paths(
        &self,
        paths: &BTreeSet<PathBuf>,
    ) -> Result<Vec<SourceImage>, SourceUnavailable> {
        match self {
            Self::Local(source) => source.iter_paths(paths),
            Self::ObjectStore(_) | Self::Unavailable(_) => Ok(Vec::new()),
        }
    }

    pub async fn iter_items(&self) -> Result<Vec<SourceImage>, SourceUnavailable> {
        self.iter_images().await
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
    id: String,
    root: PathBuf,
    extensions: BTreeSet<String>,
}

#[derive(Clone, Debug)]
pub struct ObjectStoreSource {
    id: String,
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
        id: impl Into<String>,
        root: PathBuf,
        image_extensions: BTreeSet<String>,
        audio_extensions: BTreeSet<String>,
        pdf_extensions: BTreeSet<String>,
    ) -> Self {
        let mut extensions = image_extensions;
        extensions.extend(video_extensions());
        extensions.extend(audio_extensions);
        extensions.extend(pdf_extensions);
        Self {
            id: id.into(),
            root,
            extensions,
        }
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

        iter_image_paths(&self.root, &self.extensions)
            .into_iter()
            .map(|path| self.source_image(path, &self.root))
            .collect()
    }

    fn iter_paths(
        &self,
        paths: &BTreeSet<PathBuf>,
    ) -> Result<Vec<SourceImage>, SourceUnavailable> {
        let normalized_root = self
            .root
            .canonicalize()
            .unwrap_or_else(|_| self.root.clone());
        if !self.root.exists() {
            return Ok(Vec::new());
        }

        let mut candidates = BTreeSet::new();

        for path in paths {
            let normalized_path = path.canonicalize().unwrap_or_else(|_| path.clone());
            if normalized_path != normalized_root && !normalized_path.starts_with(&normalized_root) {
                continue;
            }
            if has_hidden_component_from_root(&normalized_path, &normalized_root) {
                continue;
            }

            if normalized_path.is_file() {
                if self.supports_path(&normalized_path) {
                    candidates.insert(normalized_path);
                }
                continue;
            }

            if normalized_path.is_dir() {
                for candidate in iter_image_paths(&normalized_path, &self.extensions) {
                    let candidate = candidate.canonicalize().unwrap_or(candidate);
                    if candidate.starts_with(&normalized_root)
                        && !has_hidden_component_from_root(&candidate, &normalized_root)
                    {
                        candidates.insert(candidate);
                    }
                }
            }
        }

        candidates
            .into_iter()
            .map(|path| self.source_image(path, &normalized_root))
            .collect()
    }

    fn supports_path(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(|extension| format!(".{}", extension.to_ascii_lowercase()))
            .map(|extension| self.extensions.contains(&extension))
            .unwrap_or(false)
    }

    fn source_image(
        &self,
        path: PathBuf,
        relative_root: &Path,
    ) -> Result<SourceImage, SourceUnavailable> {
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
        Ok(SourceImage {
            source_id: self.id.clone(),
            source_type: "local".to_string(),
            source_uri: self.uri(),
            item_uri: resolved.to_string_lossy().to_string(),
            id_base: resolved.to_string_lossy().to_string(),
            display_path: resolved.to_string_lossy().to_string(),
            relative_path: relative_path(&path, relative_root),
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
        })
    }
}

fn has_hidden_component_from_root(path: &Path, root: &Path) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative.components().any(|component| match component {
        std::path::Component::Normal(name) => name
            .to_str()
            .map(|name| name.starts_with('.') && name != "." && name != "..")
            .unwrap_or(false),
        _ => false,
    })
}
