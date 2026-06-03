use std::time::Duration;

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};

use image_similarity_service::config::Settings;
use image_similarity_service::workers::media::hashing::phash_image;
use image_similarity_service::workers::media::image_io::load_media_bytes;
use image_similarity_service::workers::media::thumbnails::ensure_thumbnail;
use image_similarity_service::workers::media::visual_embedding::{
    LegacyColorEmbedder, VisualEmbeddingBackend,
};

mod support;

fn media_pipeline(c: &mut Criterion) {
    let size = support::bench_image_size(256);
    let image = support::pattern_image(size, size, [220, 40, 40], [20, 20, 20]);
    let png = support::png_bytes(&image);
    let gif = support::gif_bytes();
    let settings = Settings {
        gif_sample_frames: 4,
        gif_preview_frames: 4,
        gif_max_decode_frames: 8,
        ..Settings::default()
    };

    c.bench_function("media/static_image_decode", |b| {
        b.iter(|| {
            load_media_bytes(std::hint::black_box(&png), std::hint::black_box(&settings)).unwrap()
        })
    });

    c.bench_function("media/phash", |b| {
        b.iter(|| phash_image(std::hint::black_box(&image)));
    });

    c.bench_function("media/thumbnail_generation", |b| {
        b.iter_batched(
            support::TempDir::new,
            |dir| {
                ensure_thumbnail(
                    std::hint::black_box(&image),
                    std::hint::black_box(dir.path()),
                    std::hint::black_box("bench-image"),
                    std::hint::black_box((320, 320)),
                )
                .unwrap()
            },
            BatchSize::SmallInput,
        )
    });

    c.bench_function("media/gif_decode_sampling", |b| {
        b.iter(|| {
            load_media_bytes(std::hint::black_box(&gif), std::hint::black_box(&settings)).unwrap()
        })
    });

    let gif_media = load_media_bytes(&gif, &settings).unwrap();
    let embedder = LegacyColorEmbedder::new("bench", 64);
    c.bench_function("media/gif_vector_preparation", |b| {
        b.iter(|| {
            embedder
                .embed_media(
                    std::hint::black_box(&gif_media.sampled_frames),
                    std::hint::black_box(settings.gif_motion_weight),
                )
                .unwrap()
        })
    });

    report_external_tool_groups();
}

fn report_external_tool_groups() {
    for (group, tools) in [
        ("media/audio_external_pipeline", &["ffmpeg", "ffprobe"][..]),
        ("media/video_external_pipeline", &["ffmpeg", "ffprobe"][..]),
        (
            "media/pdf_external_pipeline",
            &["pdfinfo", "pdftoppm", "pdftotext"][..],
        ),
    ] {
        if tools.iter().any(|tool| !has_tool(tool)) {
            eprintln!("skipping {group}; missing one of {tools:?}");
        }
    }
}

fn has_tool(name: &str) -> bool {
    std::process::Command::new(name)
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
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
    targets = media_pipeline
}
criterion_main!(benches);
