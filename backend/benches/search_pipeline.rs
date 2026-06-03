use std::fs;
use std::time::Duration;

use criterion::{criterion_group, criterion_main, Criterion};

use image_similarity_service::config::parse_extensions;
use image_similarity_service::workers::duplicates::duplicate_index;
use image_similarity_service::workers::media::embedder::ImageEmbedder;
use image_similarity_service::workers::media::image_io::iter_image_paths;

mod support;

fn search_pipeline(c: &mut Criterion) {
    let item_count = support::bench_items(1_000);
    let payloads = support::synthetic_payloads(item_count);
    let query = support::synthetic_vector(42, 64);

    c.bench_function("search/query_media_embedding", |b| {
        let frames = support::media_frames(6, support::bench_image_size(128));
        let embedder = ImageEmbedder::new("bench", 64);
        b.iter(|| embedder.encode_media(std::hint::black_box(&frames), std::hint::black_box(0.2)))
    });

    let mut scored_sizes = Vec::new();
    for size in [100_usize, 1_000, 10_000] {
        let size = size.min(payloads.len());
        if size == 0 || scored_sizes.contains(&size) {
            continue;
        }
        scored_sizes.push(size);
        c.bench_function(&format!("search/filter_and_score_{size}"), |b| {
            b.iter(|| {
                support::score_payloads(
                    std::hint::black_box(&payloads[..size]),
                    std::hint::black_box(&query),
                    50,
                )
            })
        });
    }

    c.bench_function("search/duplicate_grouping", |b| {
        b.iter(|| duplicate_index(std::hint::black_box(1), std::hint::black_box(&payloads)))
    });

    c.bench_function("search/inverse_index_aggregation", |b| {
        b.iter(|| {
            let mut counts = std::collections::BTreeMap::<String, usize>::new();
            for payload in std::hint::black_box(&payloads) {
                for person in &payload.people {
                    *counts.entry(person.person_id.clone()).or_default() += 1;
                }
            }
            counts
        })
    });

    c.bench_function("search/index_planning_path_scan", |b| {
        let dir = support::TempDir::new();
        for index in 0..item_count.min(1_000) {
            let nested = dir.path().join(format!("nested-{}", index % 8));
            fs::create_dir_all(&nested).unwrap();
            fs::write(nested.join(format!("image-{index:04}.png")), b"not decoded").unwrap();
            fs::write(nested.join(format!("ignore-{index:04}.txt")), b"ignored").unwrap();
        }
        let extensions = parse_extensions(".png,.jpg").unwrap();
        b.iter(|| {
            iter_image_paths(
                std::hint::black_box(dir.path()),
                std::hint::black_box(&extensions),
            )
        })
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
    targets = search_pipeline
}
criterion_main!(benches);
