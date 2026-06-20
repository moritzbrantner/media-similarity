use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};
use serde_json::json;

use image_similarity_service::domain::models::{SearchResponse, SearchResult};
use image_similarity_service::domain::smart_albums::{
    AlbumSortMode, DuplicateStatusFilter, SmartAlbum, SmartAlbumCriteria, SmartAlbumResultsResponse,
};
use image_similarity_service::storage::MediaSearchFilter;
use image_similarity_service::workers::duplicates::duplicate_index;

mod support;

fn response_and_filters(c: &mut Criterion) {
    let item_count = support::bench_items(1_000);
    let payloads = support::synthetic_payloads(item_count);
    let duplicate_index = duplicate_index(1, &payloads);

    c.bench_function("response/filter_json_construction", |b| {
        b.iter(|| {
            let filter = MediaSearchFilter {
                source_type: Some("local".to_string()),
                media_kind: Some("static_image".to_string()),
                has_gps: Some(true),
                min_width: Some(640),
                max_width: Some(1920),
                min_height: Some(320),
                max_height: Some(1080),
                min_size_bytes: Some(100_000),
                max_size_bytes: Some(5_000_000),
                modified_from: Some(1_700_000_000.0),
                modified_to: Some(1_800_000_000.0),
                captured_from: Some(1_700_000_000.0),
                captured_to: Some(1_800_000_000.0),
            };
            json!({
                "must": [
                    { "key": "point_kind", "match": { "value": "media" } },
                    { "key": "source_type", "match": { "value": filter.source_type } },
                    { "key": "media_kind", "match": { "value": filter.media_kind } },
                    { "key": "photo_has_gps", "match": { "value": filter.has_gps } },
                    { "key": "width", "range": { "gte": filter.min_width, "lte": filter.max_width } },
                    { "key": "height", "range": { "gte": filter.min_height, "lte": filter.max_height } },
                    { "key": "size_bytes", "range": { "gte": filter.min_size_bytes, "lte": filter.max_size_bytes } },
                    { "key": "modified_at", "range": { "gte": filter.modified_from, "lte": filter.modified_to } },
                    { "key": "photo_capture_time_epoch", "range": { "gte": filter.captured_from, "lte": filter.captured_to } }
                ]
            })
        })
    });

    c.bench_function("response/search_response_serialization", |b| {
        let response = SearchResponse {
            query_phash: "0123456789abcdef".to_string(),
            count: payloads.len().min(500),
            results: payloads
                .iter()
                .take(500)
                .cloned()
                .map(|image| SearchResult {
                    image,
                    vector_score: 0.95,
                    relevance_score: Some(2.95),
                    hash_distance: Some(0),
                    ocr_score: None,
                    near_duplicate: true,
                    query_scene_index: None,
                })
                .collect(),
            query_media_kind: "static_image".to_string(),
            scenes: Vec::new(),
            query_audio_analysis: None,
            query_ocr_text: String::new(),
            query_visual_embedding_model: Some("bench".to_string()),
            query_visual_embedding_degraded: false,
        };
        b.iter(|| serde_json::to_vec(std::hint::black_box(&response)).unwrap())
    });

    c.bench_function("response/smart_album_pagination_serialization", |b| {
        let album = SmartAlbum {
            id: "album-bench".to_string(),
            name: "Bench".to_string(),
            description: None,
            criteria: SmartAlbumCriteria {
                duplicate_status: DuplicateStatusFilter::All,
                ..SmartAlbumCriteria::default()
            },
            sort: AlbumSortMode::ModifiedNewest,
            limit: 60,
            created_at: "2026-05-22T10:00:00Z".to_string(),
            updated_at: "2026-05-22T10:00:00Z".to_string(),
        };
        let response = SmartAlbumResultsResponse {
            album,
            count: payloads.len().min(60),
            total: payloads.len(),
            offset: 0,
            limit: 60,
            warnings: duplicate_index.warnings.clone(),
            duplicate_groups: duplicate_index.groups.clone(),
            results: payloads
                .iter()
                .take(60)
                .cloned()
                .map(|image| {
                    let membership = duplicate_index.by_media_id.get(&image.id);
                    image_similarity_service::domain::smart_albums::SmartAlbumResult {
                        image,
                        duplicate_group_id: membership
                            .map(|membership| membership.group_id.clone()),
                        duplicate_group_size: membership
                            .map(|membership| membership.group_size)
                            .unwrap_or(1),
                    }
                })
                .collect(),
        };
        b.iter(|| serde_json::to_vec(std::hint::black_box(&response)).unwrap())
    });
}

fn criterion_config() -> Criterion {
    let criterion = Criterion::default();
    if std::env::var("IMAGE_SIM_BENCH_QUICK").is_ok() {
        criterion
            .sample_size(10)
            .measurement_time(Duration::from_secs(1))
            .warm_up_time(Duration::from_secs(1))
    } else {
        criterion
    }
}

criterion_group! {
    name = benches;
    config = criterion_config();
    targets = response_and_filters
}
criterion_main!(benches);
