use std::collections::BTreeSet;
use std::io::ErrorKind;
use std::process::Command;

use image::{DynamicImage, ImageFormat, RgbImage};
use text_analysis_core::normalize_whitespace;
use uuid::Uuid;

use crate::config::Settings;
use crate::domain::models::{OcrAnalysis, OcrFrameText};
use crate::workers::media::media::DecodedMedia;

const MAX_OCR_IMAGE_EDGE: u32 = 2_000;

pub fn extract_media_ocr(media: &DecodedMedia, settings: &Settings) -> Result<OcrAnalysis, String> {
    if !settings.ocr_enabled {
        return Ok(OcrAnalysis::default());
    }

    let mut frames = Vec::new();
    let mut combined = Vec::new();
    let mut seen = BTreeSet::new();

    for (frame_index, frame) in media
        .sampled_frames
        .iter()
        .take(settings.ocr_max_frames)
        .enumerate()
    {
        let image = prepare_ocr_image(&frame.image);
        let text = normalize_ocr_text(&recognize_image_text(&image, settings)?);
        if text.is_empty() {
            continue;
        }

        let key = text.to_lowercase();
        if !seen.insert(key) {
            continue;
        }

        combined.push(text.clone());
        frames.push(OcrFrameText { frame_index, text });
    }

    Ok(OcrAnalysis {
        text: combined.join(" "),
        frames,
    })
}

pub fn normalize_ocr_text(text: &str) -> String {
    normalize_whitespace(text)
}

pub fn normalize_ocr_query(text: Option<&str>) -> String {
    text.map(normalize_ocr_text)
        .unwrap_or_default()
        .to_lowercase()
}

pub fn ocr_match_score(ocr_text: &str, normalized_query: &str) -> Option<f32> {
    let query = normalized_query.trim();
    if query.is_empty() {
        return Some(0.0);
    }

    let text = normalize_ocr_text(ocr_text).to_lowercase();
    if text.is_empty() {
        return None;
    }

    if text.contains(query) {
        return Some(1.0);
    }

    let terms = query
        .split_whitespace()
        .filter(|term| !term.is_empty())
        .collect::<BTreeSet<_>>();
    if terms.is_empty() {
        return Some(0.0);
    }

    let matched = terms.iter().filter(|term| text.contains(**term)).count();
    if matched == terms.len() {
        Some(matched as f32 / terms.len() as f32)
    } else {
        None
    }
}

fn recognize_image_text(image: &RgbImage, settings: &Settings) -> Result<String, String> {
    std::fs::create_dir_all(&settings.upload_dir).map_err(|error| error.to_string())?;
    let image_path = settings
        .upload_dir
        .join(format!("ocr-{}.png", Uuid::new_v4()));
    image
        .save_with_format(&image_path, ImageFormat::Png)
        .map_err(|error| error.to_string())?;

    let mut command = Command::new(&settings.ocr_command);
    command.arg(&image_path).arg("stdout");
    if let Some(language) = &settings.ocr_language {
        command.arg("-l").arg(language);
    }
    command.arg("--psm").arg("11");

    let output = command.output();
    let _ = std::fs::remove_file(&image_path);

    match output {
        Ok(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).to_string())
        }
        Ok(output) => Err(String::from_utf8_lossy(&output.stderr).trim().to_string()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(String::new()),
        Err(error) => Err(error.to_string()),
    }
}

fn prepare_ocr_image(image: &RgbImage) -> RgbImage {
    if image.width().max(image.height()) <= MAX_OCR_IMAGE_EDGE {
        return image.clone();
    }

    DynamicImage::ImageRgb8(image.clone())
        .thumbnail(MAX_OCR_IMAGE_EDGE, MAX_OCR_IMAGE_EDGE)
        .to_rgb8()
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgb};

    use super::{normalize_ocr_query, normalize_ocr_text, ocr_match_score, prepare_ocr_image};

    #[test]
    fn normalizes_ocr_whitespace() {
        assert_eq!(
            normalize_ocr_text("  Hello\n\nlarge\tworld "),
            "Hello large world"
        );
    }

    #[test]
    fn matches_exact_or_all_query_terms() {
        let text = "Total due invoice 2026";
        assert_eq!(
            ocr_match_score(text, &normalize_ocr_query(Some("total due"))),
            Some(1.0)
        );
        assert_eq!(
            ocr_match_score(text, &normalize_ocr_query(Some("invoice total"))),
            Some(1.0)
        );
        assert_eq!(
            ocr_match_score(text, &normalize_ocr_query(Some("receipt"))),
            None
        );
    }

    #[test]
    fn ocr_image_is_downscaled_before_external_recognition() {
        let image = ImageBuffer::from_pixel(4_000, 3_000, Rgb([255, 255, 255]));

        let prepared = prepare_ocr_image(&image);

        assert!(prepared.width().max(prepared.height()) <= super::MAX_OCR_IMAGE_EDGE);
        assert_eq!(prepared.dimensions(), (2_000, 1_500));
    }
}
