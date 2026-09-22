use std::fs;
use std::time::Duration;

use image_similarity_service::config::parse_extensions;
use image_similarity_service::workers::watcher::spawn_local_source_watcher;

mod support;

use support::harness::TestApp;
use support::media_fixtures::write_pattern_image;

#[tokio::test]
async fn watcher_indexes_updates_and_prunes_without_full_collection_scans() {
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png").unwrap();
        settings.duplicate_hash_distance = 0;
        settings.visual_embedding_backend = "legacy".to_string();
        settings.visual_embedding_vector_size = 32;
        settings.face_analysis_enabled = false;
        settings.ocr_enabled = false;
        settings.source_watching_enabled = true;
        settings.source_watching_debounce_ms = 100;
    })
    .await;
    let watcher = spawn_local_source_watcher(app.state.clone()).expect("watcher should be enabled");
    tokio::time::sleep(Duration::from_millis(250)).await;

    let image = app.source_path("watched.png");
    let before_create = app.qdrant_operation_counts();
    write_pattern_image(&image, 48, 48, [220, 30, 30], [35, 35, 35]);

    wait_for_payload(&app, 48, 48).await;
    wait_for_watcher_idle(&app).await;
    let after_create = app.qdrant_operation_counts();
    assert_eq!(
        after_create.unfiltered_scroll_requests, before_create.unfiltered_scroll_requests,
        "watch-triggered create should not scan the full media collection"
    );
    assert!(
        after_create.filtered_scroll_requests > before_create.filtered_scroll_requests,
        "watch-triggered create should use a source-item-filtered lookup"
    );

    tokio::time::sleep(Duration::from_millis(1100)).await;
    let before_update = app.qdrant_operation_counts();
    write_pattern_image(&image, 64, 52, [30, 160, 80], [230, 230, 40]);

    wait_for_payload(&app, 64, 52).await;
    wait_for_watcher_idle(&app).await;
    let after_update = app.qdrant_operation_counts();
    assert_eq!(
        after_update.unfiltered_scroll_requests, before_update.unfiltered_scroll_requests,
        "watch-triggered update should stay on the scoped planner"
    );
    assert_eq!(
        after_update.upserted_points,
        before_update.upserted_points + 1,
        "only the changed source item should be re-indexed"
    );

    let before_delete = app.qdrant_operation_counts();
    fs::remove_file(&image).unwrap();

    wait_for_no_payloads(&app).await;
    wait_for_watcher_idle(&app).await;
    let after_delete = app.qdrant_operation_counts();
    assert_eq!(
        after_delete.unfiltered_scroll_requests, before_delete.unfiltered_scroll_requests,
        "watch-triggered delete should not fall back to a full collection scan"
    );
    assert!(
        after_delete.deleted_points > before_delete.deleted_points,
        "the removed source item should be pruned from the index"
    );

    let jobs = app.state.jobs.snapshots().unwrap();
    assert!(
        jobs.iter()
            .any(|job| job.spec.kind.as_deref() == Some("index.watch") && job.status.is_terminal()),
        "filesystem changes should be represented by watcher indexing jobs"
    );

    watcher.abort();
}

#[tokio::test]
async fn watcher_indexes_and_prunes_a_moved_directory_subtree() {
    let app = TestApp::new(|settings| {
        settings.image_extensions = parse_extensions(".png").unwrap();
        settings.duplicate_hash_distance = 0;
        settings.visual_embedding_backend = "legacy".to_string();
        settings.visual_embedding_vector_size = 32;
        settings.face_analysis_enabled = false;
        settings.ocr_enabled = false;
        settings.source_watching_enabled = true;
        settings.source_watching_debounce_ms = 100;
    })
    .await;
    let watcher = spawn_local_source_watcher(app.state.clone()).expect("watcher should be enabled");
    tokio::time::sleep(Duration::from_millis(250)).await;

    let staged = app.root_path().join("staged-album.2026");
    fs::create_dir_all(&staged).unwrap();
    write_pattern_image(
        &staged.join("nested.png"),
        72,
        54,
        [80, 120, 220],
        [20, 20, 20],
    );
    let target = app.source_path("album.2026");
    let before_move = app.qdrant_operation_counts();
    fs::rename(&staged, &target).unwrap();

    wait_for_payload(&app, 72, 54).await;
    wait_for_watcher_idle(&app).await;
    let after_move = app.qdrant_operation_counts();
    assert_eq!(
        after_move.unfiltered_scroll_requests,
        before_move.unfiltered_scroll_requests,
        "directory moves should use source-scoped filtered planning"
    );
    assert_eq!(
        app.stored_media_payloads()[0].relative_path,
        "album.2026/nested.png"
    );

    let before_remove = app.qdrant_operation_counts();
    fs::remove_dir_all(&target).unwrap();

    wait_for_no_payloads(&app).await;
    wait_for_watcher_idle(&app).await;
    let after_remove = app.qdrant_operation_counts();
    assert_eq!(
        after_remove.unfiltered_scroll_requests,
        before_remove.unfiltered_scroll_requests,
        "directory removal should stay source-scoped"
    );

    watcher.abort();
}

async fn wait_for_payload(app: &TestApp, width: u32, height: u32) {
    for _ in 0..320 {
        let payloads = app.stored_media_payloads();
        if payloads.len() == 1 && payloads[0].width == width && payloads[0].height == height {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("watcher did not index the expected {width}x{height} payload");
}

async fn wait_for_no_payloads(app: &TestApp) {
    for _ in 0..320 {
        if app.stored_media_payloads().is_empty() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("watcher did not prune the removed source item");
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
