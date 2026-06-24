use std::fs;
use std::path::Path;
use std::process::Command;

use image::codecs::gif::{GifEncoder, Repeat};
use image::{Delay, Frame, ImageBuffer, Rgb, RgbImage};

use image_similarity_service::domain::models::{
    FaceBoxPayload, FaceDetectionPayload, FacePointPayload, ImagePayload, PersonSummary,
};
use image_similarity_service::workers::media::voice::VoiceRegistry;

use super::harness::TestApp;

pub fn test_media_payload(id: &str, filename: &str) -> ImagePayload {
    ImagePayload {
        id: id.to_string(),
        path: format!("/images/{filename}"),
        relative_path: filename.to_string(),
        filename: filename.to_string(),
        width: 100,
        height: 100,
        size_bytes: 1000,
        modified_at: 1.0,
        phash: "0000000000000000".to_string(),
        thumbnail_url: Some(format!("/thumbnails/{id}.jpg")),
        animated_thumbnail_url: None,
        media_kind: "static_image".to_string(),
        frame_count: None,
        duration_ms: None,
        full_video_url: None,
        full_audio_url: None,
        full_pdf_url: None,
        pdf_page_url: None,
        pdf_document_id: None,
        pdf_page_index: None,
        pdf_page_number: None,
        pdf_page_count: None,
        audio_analysis: None,
        ocr_text: String::new(),
        ocr_frames: Vec::new(),
        visual_embedding_model: Some("test".to_string()),
        faces: Vec::new(),
        people: Vec::new(),
        artifacts: Vec::new(),
        tags: Vec::new(),
        photo_metadata: None,
        scene_clip_url: None,
        scene_index: None,
        scene_start_frame: None,
        scene_end_frame: None,
        scene_start_seconds: None,
        scene_end_seconds: None,
        source_type: "local".to_string(),
        source_item_uri: Some(format!("local:///images/{filename}")),
        indexing_profile: Some("test".to_string()),
        source_uri: Some("local:///images".to_string()),
    }
}

pub fn test_face_detection(
    face_id: &str,
    media_id: &str,
    person_id: &str,
    label: Option<&str>,
    confidence: f32,
) -> FaceDetectionPayload {
    FaceDetectionPayload {
        face_id: face_id.to_string(),
        media_id: media_id.to_string(),
        frame_index: 0,
        bbox: FaceBoxPayload {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        confidence,
        person_id: Some(person_id.to_string()),
        person_label: label.map(ToOwned::to_owned),
    }
}

pub fn test_face_point(
    face_id: &str,
    media_id: &str,
    person_id: &str,
    label: Option<&str>,
    confidence: f32,
) -> FacePointPayload {
    FacePointPayload {
        face_id: face_id.to_string(),
        media_id: media_id.to_string(),
        frame_index: 0,
        bbox: FaceBoxPayload {
            x: 0.0,
            y: 0.0,
            width: 10.0,
            height: 10.0,
        },
        confidence,
        person_id: person_id.to_string(),
        person_label: label.map(ToOwned::to_owned),
        source_uri: Some("local:///images".to_string()),
        source_item_uri: Some(format!("local:///images/{media_id}.jpg")),
    }
}

pub fn test_person_summary(
    person_id: &str,
    label: Option<&str>,
    face_count: u32,
    confidence: f32,
) -> PersonSummary {
    PersonSummary {
        person_id: person_id.to_string(),
        label: label.map(ToOwned::to_owned),
        face_count,
        media_count: 1,
        confidence,
    }
}

pub fn seed_voice_registry(app: &TestApp) {
    let sample_rate = 8_000;
    let samples = (0..sample_rate)
        .map(|index| {
            let phase = index as f32 * 2.0 * std::f32::consts::PI * 180.0 / sample_rate as f32;
            phase.sin() * 0.2
        })
        .collect::<Vec<_>>();
    let mut registry = VoiceRegistry::load(&app.state.settings).unwrap();
    let enrolled = registry
        .recognize_or_enroll(&samples, sample_rate)
        .expect("voice should enroll");
    assert_eq!(enrolled.id, "voice-0001");
    registry.save_if_changed().unwrap();
}

pub fn write_pattern_image(path: &Path, width: u32, height: u32, a: [u8; 3], b: [u8; 3]) {
    let mut image = RgbImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let pixel = if (x / 8 + y / 8) % 2 == 0 { a } else { b };
            image.put_pixel(x, y, Rgb(pixel));
        }
    }
    image.save(path).unwrap();
}

pub fn inject_xmp_metadata(path: &Path, xmp: &str) {
    let original = fs::read(path).unwrap();
    assert_eq!(&original[..2], &[0xff, 0xd8]);
    let mut payload = b"http://ns.adobe.com/xap/1.0/\0".to_vec();
    payload.extend_from_slice(xmp.as_bytes());
    let length = payload.len() + 2;
    let mut metadata_segment = vec![0xff, 0xe1];
    metadata_segment.extend_from_slice(&(length as u16).to_be_bytes());
    metadata_segment.extend_from_slice(&payload);

    let mut updated = original[..2].to_vec();
    updated.extend_from_slice(&metadata_segment);
    updated.extend_from_slice(&original[2..]);
    fs::write(path, updated).unwrap();
}

pub fn test_photo_xmp() -> &'static str {
    r#"<x:xmpmeta xmlns:x="adobe:ns:meta/">
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:tiff="http://ns.adobe.com/tiff/1.0/" xmlns:dc="http://purl.org/dc/elements/1.1/">
<rdf:Description xmp:CreateDate="2024-03-12T10:30:00Z" tiff:Make="Acme" tiff:Model="Pocket 7">
<dc:subject><rdf:Bag><rdf:li>Travel</rdf:li><rdf:li>Sunrise</rdf:li></rdf:Bag></dc:subject>
</rdf:Description>
</rdf:RDF>
</x:xmpmeta>"#
}

pub fn write_test_gif(path: &Path, colors: &[[u8; 3]], delay_ms: u32) {
    let file = fs::File::create(path).unwrap();
    let mut encoder = GifEncoder::new(file);
    encoder.set_repeat(Repeat::Infinite).unwrap();
    let frames = colors
        .iter()
        .map(|color| {
            let image = ImageBuffer::from_pixel(32, 24, Rgb(*color));
            Frame::from_parts(
                image::DynamicImage::ImageRgb8(image).to_rgba8(),
                0,
                0,
                Delay::from_numer_denom_ms(delay_ms, 1),
            )
        })
        .collect::<Vec<_>>();
    encoder.encode_frames(frames).unwrap();
}

pub fn write_test_pdf(path: &Path, page_texts: &[&str]) {
    let mut objects = Vec::<String>::new();
    let kids = (0..page_texts.len())
        .map(|index| format!("{} 0 R", 3 + index))
        .collect::<Vec<_>>()
        .join(" ");
    objects.push("<< /Type /Catalog /Pages 2 0 R >>".to_string());
    objects.push(format!(
        "<< /Type /Pages /Kids [{kids}] /Count {} >>",
        page_texts.len()
    ));
    let font_object_id = 3 + page_texts.len();
    let first_content_id = font_object_id + 1;
    for index in 0..page_texts.len() {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 240 160] /Resources << /Font << /F1 {font_object_id} 0 R >> >> /Contents {} 0 R >>",
            first_content_id + index
        ));
    }
    objects.push("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_string());
    for text in page_texts {
        let stream = format!(
            "BT /F1 18 Tf 24 92 Td ({}) Tj ET",
            text.replace('\\', "\\\\")
                .replace('(', "\\(")
                .replace(')', "\\)")
        );
        objects.push(format!(
            "<< /Length {} >>\nstream\n{}\nendstream",
            stream.len(),
            stream
        ));
    }

    let mut pdf = Vec::from("%PDF-1.4\n".as_bytes());
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.extend_from_slice(format!("{} 0 obj\n{}\nendobj\n", index + 1, object).as_bytes());
    }
    let xref_offset = pdf.len();
    pdf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
    pdf.extend_from_slice(b"0000000000 65535 f \n");
    for offset in offsets {
        pdf.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
            objects.len() + 1
        )
        .as_bytes(),
    );
    fs::write(path, pdf).unwrap();
}

pub fn write_two_scene_video(path: &Path) {
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-v")
        .arg("error")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("color=c=red:s=64x48:d=1:r=12")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("color=c=blue:s=64x48:d=1:r=12")
        .arg("-filter_complex")
        .arg("[0:v][1:v]concat=n=2:v=1:a=0,format=yuv420p")
        .arg("-movflags")
        .arg("+faststart")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn write_voice_like_audio(path: &Path) {
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-v")
        .arg("error")
        .arg("-f")
        .arg("lavfi")
        .arg("-i")
        .arg("sine=frequency=220:duration=2.4:sample_rate=16000")
        .arg("-filter:a")
        .arg("volume=0.35")
        .arg("-ac")
        .arg("1")
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn has_tool(name: &str) -> bool {
    Command::new(name)
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
