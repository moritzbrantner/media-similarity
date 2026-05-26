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
