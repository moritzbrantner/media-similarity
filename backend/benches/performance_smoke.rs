use std::hint::black_box;

use iai_callgrind::{
    library_benchmark, library_benchmark_group, main, Callgrind, EventKind, LibraryBenchmarkConfig,
};
use image_similarity_service::domain::models::ImagePayload;
use image_similarity_service::workers::duplicates::duplicate_index;
use image_similarity_service::workers::media::embedder::ImageEmbedder;
use image_similarity_service::workers::media::media::MediaFrame;

mod support;

#[library_benchmark]
#[bench::six_frames_96px(support::media_frames(6, 96))]
fn bench_media_embedding(frames: Vec<MediaFrame>) -> Vec<f32> {
    let embedder = ImageEmbedder::new("performance-smoke", 64);
    black_box(embedder.encode_media(black_box(&frames), black_box(0.2)))
}

#[library_benchmark]
#[bench::items_64(support::synthetic_payloads(64))]
#[bench::items_128(support::synthetic_payloads(128))]
fn bench_duplicate_grouping(payloads: Vec<ImagePayload>) -> usize {
    let index = duplicate_index(black_box(1), black_box(&payloads));
    black_box(index.groups.len())
}

library_benchmark_group!(
    name = media_similarity_smoke;
    benchmarks = bench_media_embedding, bench_duplicate_grouping
);

fn benchmark_config() -> LibraryBenchmarkConfig {
    let mut callgrind = Callgrind::default();
    callgrind
        .soft_limits([(EventKind::Ir, 5.0)])
        .fail_fast(true);
    let mut config = LibraryBenchmarkConfig::default();
    config.tool(callgrind);
    config
}

main!(
    config = benchmark_config();
    library_benchmark_groups = media_similarity_smoke
);
