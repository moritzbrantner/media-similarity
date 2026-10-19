use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use image_similarity_service::api::AppState;
use image_similarity_service::app::build_app_router;
use image_similarity_service::config::{MetadataFilterProvider, Settings};
use image_similarity_service::domain::models::{IndexResponse, SearchResponse};
use serde_json::{json, Value};
use tokio::task::JoinHandle;

mod support;
use support::harness::{multipart_body, TestApp};
use support::media_fixtures::{inject_xmp_metadata, test_photo_xmp, write_pattern_image};

type Params = Vec<(&'static str, String)>;

fn browser_enabled() -> bool {
    std::env::var("METADATA_ACCEPTANCE_BROWSER").is_ok_and(|value| value == "1")
}

struct NativeApi {
    url: String,
    client: reqwest::Client,
    task: Option<JoinHandle<()>>,
}

impl NativeApi {
    async fn start(mut settings: Settings) -> Self {
        settings.frontend_serving_enabled = browser_enabled();
        fs::create_dir_all(&settings.thumbnail_dir).unwrap();
        fs::create_dir_all(&settings.upload_dir).unwrap();
        let state = Arc::new(AppState::new(settings.clone()));
        let app = build_app_router(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../frontend/dist"),
            state,
            &settings,
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            url,
            client: reqwest::Client::new(),
            task: Some(task),
        }
    }

    async fn search(&self, bytes: &[u8], params: &Params) -> SearchResponse {
        let (content_type, body) = multipart_body("query.jpg", "image/jpeg", bytes.to_vec());
        let mut url = url::Url::parse(&format!("{}/api/search", self.url)).unwrap();
        url.query_pairs_mut()
            .extend_pairs(params.iter().map(|(key, value)| (*key, value)));
        let response = self
            .client
            .post(url)
            .header(reqwest::header::CONTENT_TYPE, content_type)
            .body(body)
            .send()
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "{}",
            response.text().await.unwrap()
        );
        response.json().await.unwrap()
    }

    async fn index(&self) -> IndexResponse {
        let response = self
            .client
            .post(format!("{}/api/index", self.url))
            .json(&json!({}))
            .send()
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "{}",
            response.text().await.unwrap()
        );
        let indexed: IndexResponse = response.json().await.unwrap();
        assert_eq!(indexed.failed, 0, "{:?}", indexed.errors);
        indexed
    }

    async fn album(&self, criteria: Value) -> Value {
        let response = self
            .client
            .post(format!("{}/api/smart-albums/preview", self.url))
            .json(&json!({"name": "Metadata acceptance", "criteria": criteria}))
            .send()
            .await
            .unwrap();
        assert!(
            response.status().is_success(),
            "{}",
            response.text().await.unwrap()
        );
        let response: Value = response.json().await.unwrap();
        response["results"].clone()
    }

    async fn shutdown(mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
        }
    }

    async fn browser(&self, query: &Path, stage: &str) {
        if !browser_enabled() {
            return;
        }
        let url = self.url.clone();
        let query = query.to_owned();
        let stage = stage.to_owned();
        let output = tokio::task::spawn_blocking(move || {
            std::process::Command::new("bun")
                .arg(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("../scripts/check-metadata-ui.mjs"),
                )
                .args([url.as_str(), query.to_str().unwrap(), stage.as_str()])
                .output()
                .unwrap()
        })
        .await
        .unwrap();
        assert!(
            output.status.success(),
            "browser: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        println!("browser {}", String::from_utf8_lossy(&output.stdout));
    }
}
impl Drop for NativeApi {
    fn drop(&mut self) {
        if let Some(task) = &self.task {
            task.abort();
        }
    }
}

async fn assert_parity(
    existing: &NativeApi,
    indexed: &NativeApi,
    query: &[u8],
    queries: &[Params],
) {
    for params in queries {
        let existing = existing.search(query, params).await;
        let indexed = indexed.search(query, params).await;
        let signature = |response: SearchResponse| {
            response
                .results
                .into_iter()
                .map(|result| {
                    (
                        result.image.id,
                        result.vector_score,
                        result.relevance_score,
                        result.hash_distance,
                        result.ocr_score,
                    )
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            signature(existing),
            signature(indexed),
            "filters: {params:?}"
        );
    }
}

#[tokio::test]
async fn metadata_providers_keep_ids_scores_and_lifecycle_through_the_production_router() {
    let fixture = TestApp::new(|settings| {
        settings.face_analysis_enabled = false;
        settings.audio_transcription_enabled = false;
        settings.ocr_enabled = false;
    })
    .await;
    exercise_provider_lifecycle(fixture.state.settings.clone(), fixture.root_path()).await;
}

#[tokio::test]
#[ignore = "requires an isolated real Qdrant service through QDRANT_URL"]
async fn qdrant_smoke_metadata_providers_match_real_qdrant_through_update_remove_restart_and_rebuild(
) {
    let root = tempfile::tempdir().unwrap();
    let url =
        std::env::var("QDRANT_URL").expect("set QDRANT_URL to an isolated local test service");
    let parsed = url::Url::parse(&url).unwrap();
    assert!(
        matches!(parsed.host_str(), Some("127.0.0.1" | "localhost" | "::1")),
        "acceptance targets must be local"
    );
    let collection = format!("metadata-acceptance-{}", uuid::Uuid::new_v4());
    let settings = Settings {
        qdrant_url: url.clone(),
        qdrant_collection: collection.clone(),
        source_image_dir: root.path().join("sources"),
        image_sources: Vec::new(),
        media_sources_file: root.path().join("media-sources.txt"),
        media_sources_seed_file: None,
        thumbnail_dir: root.path().join("thumbnails"),
        upload_dir: root.path().join("uploads"),
        smart_albums_file: root.path().join("albums.json"),
        voice_registry_path: root.path().join("voices.json"),
        processing_workflows_file: root.path().join("workflows.json"),
        model_bundle_dir: root.path().join("models"),
        visual_embedding_backend: "legacy".into(),
        visual_embedding_vector_size: 32,
        face_embedding_vector_size: 32,
        default_search_limit: 10,
        face_analysis_enabled: false,
        ocr_enabled: false,
        audio_transcription_enabled: false,
        startup_indexing_enabled: false,
        source_watching_enabled: false,
        ..Default::default()
    };
    exercise_provider_lifecycle(settings, root.path()).await;
    let response = reqwest::Client::new()
        .delete(format!(
            "{}/collections/{collection}",
            url.trim_end_matches('/')
        ))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
}

async fn exercise_provider_lifecycle(mut settings: Settings, root: &Path) {
    fs::create_dir_all(&settings.source_image_dir).unwrap();
    settings.indexing_ledger_file = root.join("metadata-indexing-ledger.json");
    settings.metadata_index_dir = root.join("metadata-index");
    let dated = settings.source_image_dir.join("dated-landscape.jpg");
    let portrait = settings.source_image_dir.join("portrait.jpg");
    let missing = settings.source_image_dir.join("missing-date.jpg");
    write_pattern_image(&dated, 80, 40, [220, 30, 30], [30, 30, 30]);
    write_pattern_image(&portrait, 40, 80, [30, 80, 220], [30, 30, 30]);
    write_pattern_image(&missing, 80, 40, [30, 180, 90], [30, 30, 30]);
    inject_xmp_metadata(&dated, test_photo_xmp());
    inject_xmp_metadata(&portrait, test_photo_xmp());
    let query = fs::read(&dated).unwrap();
    let browser_query = root.join("query.jpg");
    fs::write(&browser_query, &query).unwrap();
    settings.metadata_filter_provider = MetadataFilterProvider::Existing;
    let existing = NativeApi::start(settings.clone()).await;
    assert_eq!(existing.index().await.indexed, 3);
    settings.metadata_filter_provider = MetadataFilterProvider::SearchEngine;
    let indexed = NativeApi::start(settings.clone()).await;
    let queries = vec![
        vec![],
        vec![("min_width", "80".into()), ("max_height", "40".into())],
        vec![
            ("captured_from", "1710239400".into()),
            ("captured_to", "1710239400".into()),
            ("min_width", "80".into()),
        ],
        vec![
            ("name_query", "DATED".into()),
            ("camera_query", "Pocket".into()),
        ],
        vec![("media_kind", "audio".into())],
        vec![("name_query", "portrait".into())],
    ];
    assert_parity(&existing, &indexed, &query, &queries).await;
    existing.browser(&browser_query, "initial").await;
    indexed.browser(&browser_query, "initial").await;
    let date_query = &queries[2];
    let matches = indexed.search(&query, date_query).await;
    assert_eq!(matches.count, 1);
    assert_eq!(matches.results[0].image.filename, "dated-landscape.jpg");
    let stable_id = matches.results[0].image.id.clone();
    let criteria = json!({"min_width": 80, "captured_from": 1710239400.0});
    assert_eq!(
        existing.album(criteria.clone()).await,
        indexed.album(criteria.clone()).await
    );

    indexed.index().await;
    assert_parity(&existing, &indexed, &query, &queries).await;
    assert_eq!(
        indexed.search(&query, date_query).await.results[0].image.id,
        stable_id
    );

    write_pattern_image(&dated, 20, 10, [220, 30, 30], [30, 30, 30]);
    fs::remove_file(&portrait).unwrap();
    indexed.index().await;
    assert_parity(&existing, &indexed, &query, &queries).await;
    assert_eq!(indexed.search(&query, date_query).await.count, 0);
    assert_eq!(indexed.search(&query, &queries[5]).await.count, 0);
    existing.browser(&browser_query, "updated").await;
    indexed.browser(&browser_query, "updated").await;
    assert!(existing
        .search(&query, &vec![])
        .await
        .results
        .iter()
        .any(|payload| payload.image.id == stable_id && payload.image.width == 20));

    indexed.shutdown().await;
    let reopened = NativeApi::start(settings.clone()).await;
    assert_parity(&existing, &reopened, &query, &queries).await;
    reopened.browser(&browser_query, "reopened").await;
    reopened.shutdown().await;
    fs::remove_dir_all(&settings.metadata_index_dir).unwrap();
    let rebuilt = NativeApi::start(settings).await;
    assert_parity(&existing, &rebuilt, &query, &queries).await;
    rebuilt.browser(&browser_query, "rebuilt").await;
    assert_eq!(
        existing.album(criteria.clone()).await,
        rebuilt.album(criteria).await
    );
    rebuilt.shutdown().await;
    existing.shutdown().await;
}
