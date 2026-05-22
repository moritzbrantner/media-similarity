use std::collections::HashSet;
use std::env;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::Duration;

use image::ImageFormat;
use reqwest::header::{ACCEPT, CACHE_CONTROL, PRAGMA, USER_AGENT};
use url::Url;
use uuid::Uuid;

const DEFAULT_FACE_URL: &str = "https://thispersondoesnotexist.com/";
const DEFAULT_FACE_COUNT: u32 = 150;
const DEFAULT_DOWNLOAD_DELAY_MS: u64 = 1_000;
const DEFAULT_MAX_ATTEMPTS_MULTIPLIER: u32 = 5;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = path_var("SAMPLE_FACE_DATA_DIR", "DUMMY_DATA_DIR", "sample-images");
    let count = positive_int_var("SAMPLE_FACE_COUNT", "DUMMY_IMAGE_COUNT", DEFAULT_FACE_COUNT)?;
    let source_url = string_var("SAMPLE_FACE_URL", DEFAULT_FACE_URL);
    let delay_ms = non_negative_int_var("SAMPLE_FACE_DELAY_MS", DEFAULT_DOWNLOAD_DELAY_MS)?;
    let max_attempts = positive_int_var(
        "SAMPLE_FACE_MAX_ATTEMPTS",
        "",
        count.saturating_mul(DEFAULT_MAX_ATTEMPTS_MULTIPLIER),
    )?;
    let clear_generated = bool_var("SAMPLE_FACE_CLEAR_GENERATED", true);

    std::fs::create_dir_all(&output_dir)?;
    if clear_generated {
        clear_previous_generated_files(&output_dir)?;
    }

    let client = reqwest::Client::builder()
        .user_agent("image-similarity-service-seed/0.1")
        .build()?;
    let mut seen_hashes = HashSet::new();
    let mut saved = 0_u32;
    let mut attempts = 0_u32;

    while saved < count && attempts < max_attempts {
        attempts += 1;
        match download_face(&client, &source_url, saved + 1, attempts).await {
            Ok(bytes) => {
                let hash = content_hash(&bytes);
                if seen_hashes.insert(hash) {
                    saved += 1;
                    let path = output_dir.join(format!("person-{saved:03}.jpg"));
                    std::fs::write(&path, bytes)?;
                    println!("Saved {}", path.display());
                } else {
                    eprintln!("Skipped duplicate response on attempt {attempts}");
                }
            }
            Err(error) => {
                eprintln!("Download attempt {attempts} failed: {error}");
            }
        }

        if saved < count && delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        }
    }

    if saved != count {
        return Err(format!(
            "Downloaded {saved} unique face images after {attempts} attempts; expected {count}"
        )
        .into());
    }

    println!(
        "Downloaded {saved} example people images from {source_url} into {}",
        output_dir.display()
    );
    Ok(())
}

async fn download_face(
    client: &reqwest::Client,
    source_url: &str,
    index: u32,
    attempt: u32,
) -> Result<Vec<u8>, String> {
    let url = cache_busted_url(source_url, index, attempt);
    let response = client
        .get(&url)
        .header(ACCEPT, "image/jpeg,image/*;q=0.8")
        .header(CACHE_CONTROL, "no-cache")
        .header(PRAGMA, "no-cache")
        .header(USER_AGENT, "image-similarity-service-seed/0.1")
        .send()
        .await
        .map_err(|error| error.to_string())?;
    let status = response.status();
    if !status.is_success() {
        return Err(format!("GET {url} returned {status}"));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|error| error.to_string())?
        .to_vec();
    let format = image::guess_format(&bytes).map_err(|error| error.to_string())?;
    if format != ImageFormat::Jpeg {
        return Err(format!("GET {url} returned {format:?}, expected JPEG"));
    }
    Ok(bytes)
}

fn cache_busted_url(source_url: &str, index: u32, attempt: u32) -> String {
    let token = format!("{index}-{attempt}-{}", Uuid::new_v4());
    match Url::parse(source_url) {
        Ok(mut url) => {
            url.query_pairs_mut().append_pair("seed", &token);
            url.to_string()
        }
        Err(_) => source_url.to_string(),
    }
}

fn content_hash(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

fn clear_previous_generated_files(output_dir: &Path) -> Result<(), std::io::Error> {
    for entry in std::fs::read_dir(output_dir)? {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let is_generated_face = file_name.starts_with("person-") && file_name.ends_with(".jpg");
        let is_legacy_dummy = file_name.starts_with("dummy-")
            && [".jpg", ".jpeg", ".png", ".webp", ".gif"]
                .iter()
                .any(|extension| file_name.ends_with(extension));
        if is_generated_face || is_legacy_dummy {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn path_var(primary_name: &str, legacy_name: &str, default: &str) -> PathBuf {
    optional_string_var(primary_name)
        .or_else(|| optional_string_var(legacy_name))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(default))
}

fn string_var(name: &str, default: &str) -> String {
    optional_string_var(name).unwrap_or_else(|| default.to_string())
}

fn optional_string_var(name: &str) -> Option<String> {
    if name.is_empty() {
        return None;
    }
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn positive_int_var(primary_name: &str, legacy_name: &str, default: u32) -> Result<u32, String> {
    let value = optional_string_var(primary_name).or_else(|| optional_string_var(legacy_name));
    match value {
        Some(value) => {
            let parsed = value
                .parse::<u32>()
                .map_err(|_| format!("{primary_name} must be a positive integer"))?;
            if parsed < 1 {
                Err(format!("{primary_name} must be a positive integer"))
            } else {
                Ok(parsed)
            }
        }
        None => Ok(default),
    }
}

fn non_negative_int_var(name: &str, default: u64) -> Result<u64, String> {
    match optional_string_var(name) {
        Some(value) => value
            .parse::<u64>()
            .map_err(|_| format!("{name} must be a non-negative integer")),
        None => Ok(default),
    }
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
