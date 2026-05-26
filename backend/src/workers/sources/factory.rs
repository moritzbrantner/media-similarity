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

pub fn video_extensions() -> BTreeSet<String> {
    [".mp4", ".mov", ".m4v", ".webm", ".mkv", ".avi"]
        .into_iter()
        .map(ToOwned::to_owned)
        .collect()
}

#[allow(dead_code)]
fn unavailable_image(error: String) -> SourceLoader {
    SourceLoader::Unavailable(error)
}
