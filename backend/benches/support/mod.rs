#![allow(dead_code)]

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, ImageBuffer, ImageFormat, Rgb, RgbImage};
use uuid::Uuid;

use image_similarity_service::domain::models::{
    ImagePayload, OcrFrameText, PersonSummary, PhotoGpsPayload, PhotoMetadataPayload,
};
use image_similarity_service::workers::media::media::MediaFrame;

pub fn bench_items(default: usize) -> usize {
    if std::env::var("IMAGE_SIM_BENCH_QUICK").is_ok() {
        return default.min(100);
    }
    std::env::var("IMAGE_SIM_BENCH_ITEMS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

pub fn bench_image_size(default: u32) -> u32 {
    if std::env::var("IMAGE_SIM_BENCH_QUICK").is_ok() {
        return default.min(128);
    }
    std::env::var("IMAGE_SIM_BENCH_IMAGE_SIZE")
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(default)
}

pub fn pattern_image(width: u32, height: u32, a: [u8; 3], b: [u8; 3]) -> RgbImage {
    let mut image = RgbImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let pixel = if (x / 8 + y / 8) % 2 == 0 { a } else { b };
            image.put_pixel(x, y, Rgb(pixel));
        }
    }
    image
}

pub fn png_bytes(image: &RgbImage) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image.clone())
        .write_to(&mut cursor, ImageFormat::Png)
        .unwrap();
    cursor.into_inner()
}

pub fn gif_bytes() -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = GifEncoder::new(&mut bytes);
        encoder.set_repeat(Repeat::Infinite).unwrap();
        let frames = [[220, 40, 40], [40, 220, 40], [40, 40, 220], [220, 220, 40]]
            .into_iter()
            .map(|color| {
                let image = ImageBuffer::from_pixel(96, 72, Rgb(color));
                Frame::from_parts(
                    image::DynamicImage::ImageRgb8(image).to_rgba8(),
                    0,
                    0,
                    Delay::from_numer_denom_ms(60, 1),
                )
            });
        encoder.encode_frames(frames).unwrap();
    }
    bytes
}

pub fn media_frames(count: usize, size: u32) -> Vec<MediaFrame> {
    (0..count)
        .map(|index| MediaFrame {
            image: pattern_image(
                size,
                size,
                [((index * 31) % 255) as u8, 60, 120],
                [20, ((index * 47) % 255) as u8, 220],
            ),
            delay_ms: 80 + index as u32,
        })
        .collect()
}

pub fn synthetic_payload(index: usize) -> ImagePayload {
    let media_kind = match index % 5 {
        0 => "static_image",
        1 => "animated_gif",
        2 => "video_scene",
        3 => "audio",
        _ => "pdf_page",
    };
    let source_type = if index.is_multiple_of(7) {
        "s3"
    } else {
        "local"
    };
    ImagePayload {
        id: format!("media-{index:06}"),
        path: format!("/bench/source-{}/media-{index:06}.jpg", index % 4),
        relative_path: format!("source-{}/media-{index:06}.jpg", index % 4),
        filename: format!("media-{index:06}.jpg"),
        width: 640 + (index % 5) as u32 * 32,
        height: 480 + (index % 3) as u32 * 24,
        size_bytes: 100_000 + index as u64 * 17,
        modified_at: 1_700_000_000.0 + index as f64,
        phash: format!("{:016x}", index as u64 / 2),
        thumbnail_url: Some(format!("/thumbnails/media-{index:06}.jpg")),
        animated_thumbnail_url: (media_kind == "animated_gif")
            .then(|| format!("/thumbnails/media-{index:06}.gif")),
        media_kind: media_kind.to_string(),
        frame_count: (media_kind == "animated_gif").then_some(4),
        duration_ms: (media_kind == "animated_gif").then_some(240),
        full_video_url: (media_kind == "video_scene")
            .then(|| format!("/uploads/media-{index:06}.mp4")),
        full_audio_url: (media_kind == "audio").then(|| format!("/uploads/media-{index:06}.mp3")),
        full_pdf_url: media_kind
            .starts_with("pdf")
            .then(|| format!("/uploads/media-{index:06}.pdf")),
        pdf_page_url: (media_kind == "pdf_page")
            .then(|| format!("/uploads/media-{index:06}.pdf#page=1")),
        pdf_document_id: (media_kind == "pdf_page").then(|| format!("pdf-doc-{index:06}")),
        pdf_page_index: (media_kind == "pdf_page").then_some(0),
        pdf_page_number: (media_kind == "pdf_page").then_some(1),
        pdf_page_count: media_kind.starts_with("pdf").then_some(2),
        audio_analysis: None,
        ocr_text: if index.is_multiple_of(11) {
            "Invoice total due".to_string()
        } else {
            String::new()
        },
        ocr_frames: vec![OcrFrameText {
            frame_index: 0,
            text: format!("frame text {index}"),
        }],
        visual_embedding_model: Some("legacy-disabled".to_string()),
        faces: Vec::new(),
        people: vec![PersonSummary {
            person_id: format!("person-{:03}", index % 16),
            label: None,
            face_count: 1,
            media_count: 1,
            confidence: 0.8,
        }],
        artifacts: Vec::new(),
        tags: vec![format!("tag-{}", index % 8)],
        photo_metadata: Some(PhotoMetadataPayload {
            camera_make: Some("BenchCam".to_string()),
            camera_model: Some(format!("Model {}", index % 3)),
            gps: index.is_multiple_of(2).then_some(PhotoGpsPayload {
                latitude: 52.0,
                longitude: 13.0,
                altitude_meters: None,
            }),
            keywords: vec![format!("keyword-{}", index % 5)],
            capture_time: Some("2024-03-12T10:30:00Z".to_string()),
            ..PhotoMetadataPayload::default()
        }),
        scene_clip_url: (media_kind == "video_scene")
            .then(|| format!("/uploads/media-{index:06}-scene.mp4")),
        scene_index: (media_kind == "video_scene").then_some(index % 3),
        scene_start_frame: (media_kind == "video_scene").then_some(0),
        scene_end_frame: (media_kind == "video_scene").then_some(120),
        scene_start_seconds: (media_kind == "video_scene").then_some(0.0),
        scene_end_seconds: (media_kind == "video_scene").then_some(4.0),
        source_type: source_type.to_string(),
        source_item_uri: Some(format!("{source_type}://bench/media-{index:06}")),
        indexing_profile: Some("bench-profile".to_string()),
        source_uri: Some(format!("{source_type}://bench/source-{}", index % 4)),
    }
}

pub fn synthetic_payloads(count: usize) -> Vec<ImagePayload> {
    (0..count).map(synthetic_payload).collect()
}

pub fn score_payloads(
    payloads: &[ImagePayload],
    query: &[f32],
    limit: usize,
) -> Vec<(String, f32)> {
    let mut scored = payloads
        .iter()
        .enumerate()
        .filter(|(_, payload)| {
            payload.media_kind == "static_image" || payload.media_kind == "pdf_page"
        })
        .map(|(index, payload)| {
            let vector = synthetic_vector(index, query.len());
            (payload.id.clone(), cosine_similarity(query, &vector))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| right.1.total_cmp(&left.1));
    scored.truncate(limit);
    scored
}

pub fn synthetic_vector(seed: usize, len: usize) -> Vec<f32> {
    let mut vector = (0..len)
        .map(|index| (((seed + 1) * (index + 3)) % 97) as f32 / 97.0)
        .collect::<Vec<_>>();
    normalize(&mut vector);
    vector
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    let len = left.len().min(right.len());
    let mut dot = 0.0_f32;
    for index in 0..len {
        dot += left[index] * right[index];
    }
    dot
}

fn normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in vector {
            *value /= norm;
        }
    }
}

pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!("image-sim-bench-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}
