//! Acceptance for the owner decision on PR #66 (option B, 2026-10-10): media points written
//! before `source_item_uri`/`source_uri` existed are backfilled once, with those fields derived
//! like `legacy_source_item_uri`, and the full-rescan guard keeps watcher indexing correct until
//! the backfill has finished. After it, scoped watcher indexing is complete and stays scoped.
//!
//! Oracle: a file indexed by the current code gets the modern payload. Clearing its two source
//! identity fields gives the legacy payload, and a correct backfill restores the modern one.

use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use image_similarity_service::config::parse_extensions;
use image_similarity_service::domain::models::{FacePointPayload, ImagePayload};
use image_similarity_service::storage::{MediaVectorStore, ScoredPoint, StoredPoint};
use image_similarity_service::workers::indexing::backfill::backfill_legacy_source_identity;
use image_similarity_service::workers::watcher::spawn_local_source_watcher;

mod support;

use support::harness::TestApp;
use support::media_fixtures::write_pattern_image;

async fn watched_app() -> TestApp {
    TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png").unwrap();
        settings.duplicate_hash_distance = 0;
        settings.visual_embedding_backend = "legacy".to_string();
        settings.visual_embedding_vector_size = 32;
        settings.face_analysis_enabled = false;
        settings.ocr_enabled = false;
        settings.source_watching_enabled = true;
        settings.source_watching_debounce_ms = 100;
    })
    .await
}

/// Indexes the named files with the current code and returns their modern payloads.
async fn index_modern(app: &TestApp, files: &[(&str, u32, u32)]) -> Vec<ImagePayload> {
    for (name, width, height) in files {
        write_pattern_image(
            &app.source_path(name),
            *width,
            *height,
            [200, 40, 40],
            [30, 30, 30],
        );
    }
    app.index().await;
    let payloads = app.stored_media_payloads();
    assert_eq!(
        payloads.len(),
        files.len(),
        "each fixture file is one media point"
    );
    for payload in &payloads {
        assert!(payload.source_item_uri.is_some() && payload.source_uri.is_some());
    }
    payloads
}

fn legacy(payload: &ImagePayload) -> ImagePayload {
    ImagePayload {
        source_item_uri: None,
        source_uri: None,
        ..payload.clone()
    }
}

/// A second, stale legacy point of the same source item (for example a page or scene that a
/// newer analysis no longer produces). Its path carries a generated fragment.
fn stale_legacy_sibling(payload: &ImagePayload) -> ImagePayload {
    ImagePayload {
        id: format!("{}-stale", payload.id),
        path: format!("{}#page=2", payload.path),
        ..legacy(payload)
    }
}

async fn wait_until(app: &TestApp, what: &str, done: impl Fn(&[ImagePayload]) -> bool) {
    for _ in 0..400 {
        if done(&app.stored_media_payloads()) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!(
        "timed out waiting until {what}; payloads: {:#?}",
        app.stored_media_payloads()
    );
}

async fn wait_for_backfill(app: &TestApp) {
    wait_until(
        app,
        "every media point has its source identity",
        |payloads| {
            !payloads.is_empty()
                && payloads.iter().all(|payload| {
                    payload.source_item_uri.is_some() && payload.source_uri.is_some()
                })
        },
    )
    .await;
}

async fn wait_for_watcher_idle(app: &TestApp) {
    let mut stable_polls = 0;
    let mut previous_watch_jobs = 0;
    for _ in 0..320 {
        let jobs = app.state.jobs.snapshots().unwrap();
        let watch_jobs = jobs
            .iter()
            .filter(|job| job.spec.kind.as_deref() == Some("index.watch"))
            .collect::<Vec<_>>();
        let all_terminal = watch_jobs.iter().all(|job| job.status.is_terminal());
        if all_terminal && watch_jobs.len() == previous_watch_jobs {
            stable_polls += 1;
            if stable_polls >= 8 {
                return;
            }
        } else {
            stable_polls = 0;
            previous_watch_jobs = watch_jobs.len();
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("watcher indexing jobs did not become idle");
}

#[tokio::test]
async fn backfills_legacy_source_identity_once_and_leaves_modern_points_alone() {
    let app = watched_app().await;
    let modern = index_modern(&app, &[("legacy.png", 48, 48), ("modern.png", 40, 40)]).await;
    let (legacy_original, modern_point) = (&modern[0], &modern[1]);
    assert_eq!(legacy_original.filename, "legacy.png");
    app.seed_media_payload(legacy(legacy_original)).await;
    app.seed_media_payload(stale_legacy_sibling(legacy_original))
        .await;

    let watcher = spawn_local_source_watcher(app.state.clone()).expect("watcher should be enabled");
    wait_for_backfill(&app).await;

    let payloads = app.stored_media_payloads();
    let restored = payloads
        .iter()
        .find(|payload| payload.id == legacy_original.id)
        .expect("the legacy point is kept, not replaced");
    // Derived like `legacy_source_item_uri`: the backfill restores exactly the modern payload.
    assert_eq!(restored, legacy_original);
    let sibling = payloads
        .iter()
        .find(|payload| payload.id.ends_with("-stale"))
        .expect("backfill does not delete points");
    assert_eq!(sibling.source_item_uri, legacy_original.source_item_uri);
    assert_eq!(sibling.source_uri, legacy_original.source_uri);
    assert_eq!(
        payloads
            .iter()
            .find(|payload| payload.id == modern_point.id)
            .unwrap(),
        modern_point,
        "points that already carry their identity are not rewritten"
    );

    watcher.abort();
}

#[tokio::test]
async fn backfill_and_scoped_watching_never_scan_the_whole_collection() {
    let app = watched_app().await;
    let modern = index_modern(&app, &[("legacy.png", 48, 48)]).await;
    app.seed_media_payload(legacy(&modern[0])).await;

    let before_watcher = app.qdrant_operation_counts();
    let watcher = spawn_local_source_watcher(app.state.clone()).expect("watcher should be enabled");
    wait_for_backfill(&app).await;
    wait_for_watcher_idle(&app).await;

    // Once backfilled, a delete is handled by the scoped planner and prunes the point.
    fs::remove_file(app.source_path("legacy.png")).unwrap();
    wait_until(&app, "the deleted file's point is pruned", |payloads| {
        payloads.is_empty()
    })
    .await;
    wait_for_watcher_idle(&app).await;

    let after = app.qdrant_operation_counts();
    assert_eq!(
        after.unfiltered_scroll_requests, before_watcher.unfiltered_scroll_requests,
        "the backfill reads only legacy points and post-backfill watching stays scoped"
    );
    assert!(after.deleted_points > before_watcher.deleted_points);

    watcher.abort();
}

#[tokio::test]
async fn deleting_a_file_prunes_its_legacy_points_even_before_the_backfill_finishes() {
    let app = watched_app().await;
    let modern = index_modern(&app, &[("legacy.png", 48, 48)]).await;
    app.seed_media_payload(legacy(&modern[0])).await;
    app.seed_media_payload(stale_legacy_sibling(&modern[0]))
        .await;

    let watcher = spawn_local_source_watcher(app.state.clone()).expect("watcher should be enabled");
    tokio::time::sleep(Duration::from_millis(250)).await;
    fs::remove_file(app.source_path("legacy.png")).unwrap();

    wait_until(
        &app,
        "both legacy points of the deleted file are pruned",
        |payloads| payloads.is_empty(),
    )
    .await;

    watcher.abort();
}

#[tokio::test]
async fn an_update_that_yields_fewer_points_prunes_the_stale_legacy_ones() {
    let app = watched_app().await;
    let modern = index_modern(&app, &[("legacy.png", 48, 48)]).await;
    app.seed_media_payload(legacy(&modern[0])).await;
    app.seed_media_payload(stale_legacy_sibling(&modern[0]))
        .await;

    let watcher = spawn_local_source_watcher(app.state.clone()).expect("watcher should be enabled");
    tokio::time::sleep(Duration::from_millis(1100)).await;
    write_pattern_image(
        &app.source_path("legacy.png"),
        64,
        52,
        [30, 160, 80],
        [230, 230, 40],
    );

    wait_until(&app, "only the re-indexed point remains", |payloads| {
        payloads.len() == 1 && payloads[0].width == 64 && payloads[0].height == 52
    })
    .await;
    wait_for_watcher_idle(&app).await;
    let payloads = app.stored_media_payloads();
    assert_eq!(
        payloads.len(),
        1,
        "the stale legacy sibling must not stay searchable"
    );
    assert_eq!(payloads[0].source_item_uri, modern[0].source_item_uri);

    watcher.abort();
}

/// Delegates to the app's real Qdrant store and, right before forwarding the backfill's write for
/// `point_id`, upserts `modern` for it: an index write that modernized the point between the
/// backfill's legacy scroll and its conditional write.
struct ModernizeBeforeBackfillWrite {
    inner: Arc<dyn MediaVectorStore>,
    modern: ImagePayload,
    vector_size: usize,
    races: AtomicUsize,
}

#[async_trait]
impl MediaVectorStore for ModernizeBeforeBackfillWrite {
    async fn ensure_collection(&self) -> Result<(), String> {
        self.inner.ensure_collection().await
    }

    async fn upsert_media(&self, payload: &ImagePayload, vector: Vec<f32>) -> Result<(), String> {
        self.inner.upsert_media(payload, vector).await
    }

    async fn upsert_face(
        &self,
        payload: &FacePointPayload,
        vector: Vec<f32>,
    ) -> Result<(), String> {
        self.inner.upsert_face(payload, vector).await
    }

    async fn set_media_payload(&self, payload: &ImagePayload) -> Result<(), String> {
        self.inner.set_media_payload(payload).await
    }

    async fn set_face_payload(&self, payload: &FacePointPayload) -> Result<(), String> {
        self.inner.set_face_payload(payload).await
    }

    async fn delete_points(&self, ids: &[String]) -> Result<(), String> {
        self.inner.delete_points(ids).await
    }

    async fn search_visual(
        &self,
        vector: Vec<f32>,
        limit: u32,
    ) -> Result<Vec<ScoredPoint>, String> {
        self.inner.search_visual(vector, limit).await
    }

    async fn search_faces(&self, vector: Vec<f32>, limit: u32) -> Result<Vec<ScoredPoint>, String> {
        self.inner.search_faces(vector, limit).await
    }

    async fn scroll_media_points(&self) -> Result<Vec<StoredPoint>, String> {
        self.inner.scroll_media_points().await
    }

    async fn scroll_face_points(&self) -> Result<Vec<StoredPoint>, String> {
        self.inner.scroll_face_points().await
    }

    async fn scroll_media_points_by_filter(
        &self,
        id: Option<&str>,
        source_uri: Option<&str>,
        source_item_uri: Option<&str>,
    ) -> Result<Vec<StoredPoint>, String> {
        self.inner
            .scroll_media_points_by_filter(id, source_uri, source_item_uri)
            .await
    }

    async fn scroll_legacy_media_points(
        &self,
        limit: Option<u32>,
    ) -> Result<Vec<StoredPoint>, String> {
        self.inner.scroll_legacy_media_points(limit).await
    }

    async fn set_legacy_media_source_identity(
        &self,
        point_id: &str,
        source_item_uri: &str,
        source_uri: Option<&str>,
    ) -> Result<(), String> {
        if point_id == self.modern.id {
            self.races.fetch_add(1, Ordering::SeqCst);
            self.inner
                .upsert_media(&self.modern, vec![0.0; self.vector_size])
                .await?;
        }
        self.inner
            .set_legacy_media_source_identity(point_id, source_item_uri, source_uri)
            .await
    }

    async fn scroll_face_points_by_media_ids(
        &self,
        media_ids: &[String],
    ) -> Result<Vec<StoredPoint>, String> {
        self.inner.scroll_face_points_by_media_ids(media_ids).await
    }
}

/// Issue #74: the backfill's write is conditional (`has_id`, `point_kind = media`, `is_empty
/// source_item_uri`), so a point that a concurrent index write modernized after the backfill's
/// scroll keeps its fresh payload. A write selected by `points` only, or one without the
/// `is_empty` condition, would put the identity derived from the stale legacy path back.
#[tokio::test]
async fn backfill_never_overwrites_a_point_modernized_after_its_scroll() {
    let app = watched_app().await;
    let modern = index_modern(&app, &[("legacy.png", 48, 48)]).await;
    let original = &modern[0];
    app.seed_media_payload(legacy(original)).await;

    // The concurrent index write saw the item under a new name, so its identity differs from the
    // one the backfill derives from the stale legacy path.
    let rename = |value: &str| value.replace("legacy.png", "renamed.png");
    let modernized = ImagePayload {
        path: rename(&original.path),
        relative_path: rename(&original.relative_path),
        filename: "renamed.png".to_string(),
        source_item_uri: original.source_item_uri.as_deref().map(rename),
        ..original.clone()
    };
    assert_ne!(modernized.source_item_uri, original.source_item_uri);
    let store = ModernizeBeforeBackfillWrite {
        inner: app.state.store.clone(),
        modern: modernized.clone(),
        vector_size: app.state.settings.visual_embedding_vector_size,
        races: AtomicUsize::new(0),
    };

    let outcome = backfill_legacy_source_identity(&store, &app.state.settings)
        .await
        .expect("the backfill pass runs");

    assert_eq!(
        store.races.load(Ordering::SeqCst),
        1,
        "the backfill scrolled the legacy point and then wrote it"
    );
    assert_eq!(outcome.failed_writes, 0);
    assert_eq!(outcome.unreadable, 0);
    assert_eq!(
        app.stored_media_payloads(),
        vec![modernized],
        "the concurrently modernized payload is kept exactly"
    );
}
