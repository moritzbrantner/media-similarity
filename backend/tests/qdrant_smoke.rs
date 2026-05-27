use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image::{Rgb, RgbImage};
use uuid::Uuid;

use image_similarity_service::api::AppState;
use image_similarity_service::config::{parse_extensions, Settings};
use image_similarity_service::workers::indexer::ImageIndexer;
use image_similarity_service::workers::media::image_io::load_media_bytes;
use image_similarity_service::workers::search::ImageSearchService;

#[tokio::test]
#[ignore = "requires a running Qdrant service"]
async fn qdrant_smoke_indexes_and_searches_static_image() {
    let root = TempDir::new();
    let source_dir = root.path().join("sources");
    let thumbnail_dir = root.path().join("thumbnails");
    let upload_dir = root.path().join("uploads");
    fs::create_dir_all(&source_dir).unwrap();
    fs::create_dir_all(&thumbnail_dir).unwrap();
    fs::create_dir_all(&upload_dir).unwrap();

    let mut settings = Settings {
        source_image_dir: source_dir.clone(),
        qdrant_url: std::env::var("QDRANT_URL")
            .unwrap_or_else(|_| "http://qdrant:6333".to_string()),
        qdrant_collection: format!("smoke-{}", Uuid::new_v4()),
        qdrant_request_timeout_ms: 5_000,
        qdrant_connect_timeout_ms: 1_000,
        qdrant_retry_attempts: 2,
        qdrant_retry_backoff_ms: 100,
        thumbnail_dir,
        upload_dir,
        voice_registry_path: root.path().join("recognized-voices.json"),
        media_sources_file: root.path().join("config/media-sources.txt"),
        vector_size: 32,
        visual_embedding_backend: "legacy".to_string(),
        visual_embedding_vector_size: 32,
        face_embedding_vector_size: 32,
        default_search_limit: 5,
        duplicate_hash_distance: 8,
        ocr_enabled: false,
        image_sources: Vec::new(),
        image_extensions: parse_extensions(".png").unwrap(),
        ..Settings::default()
    };
    settings.face_analysis_enabled = false;
    settings.audio_transcription_enabled = false;

    let source = source_dir.join("smoke-static.png");
    write_pattern_image(&source, 64, 40, [220, 20, 20], [20, 20, 20]);

    let state = Arc::new(AppState::new(settings.clone()));
    let indexer = ImageIndexer::new(
        settings.clone(),
        state.store.clone(),
        state.embedder.clone(),
    );
    let indexed = indexer.index_sources().await;
    assert_eq!(indexed.indexed, 1, "{:?}", indexed.errors);
    assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);
    assert_eq!(indexed.collection, settings.qdrant_collection);

    let bytes = fs::read(&source).unwrap();
    let query_media = load_media_bytes(&bytes, &settings).unwrap();
    let search = ImageSearchService::new(settings, state.store.clone(), state.embedder.clone());
    let response = search
        .search_media(&query_media, Some(1), None, None)
        .await
        .unwrap();

    assert_eq!(response.count, 1);
    assert_eq!(response.results[0].image.filename, "smoke-static.png");
    assert_eq!(response.results[0].image.id, image_id_for_path(&source));
}

struct TempDir {
    path: PathBuf,
}

impl TempDir {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("image-sim-qdrant-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn write_pattern_image(path: &Path, width: u32, height: u32, a: [u8; 3], b: [u8; 3]) {
    let mut image = RgbImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let pixel = if (x / 8 + y / 8) % 2 == 0 { a } else { b };
            image.put_pixel(x, y, Rgb(pixel));
        }
    }
    image.save(path).unwrap();
}

fn image_id_for_path(path: &Path) -> String {
    let resolved = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    image_similarity_service::workers::media::image_io::image_id_for_uri(
        &resolved.to_string_lossy(),
    )
}
