use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

use image::RgbImage;
use text_analysis_core::normalize_whitespace;
use uuid::Uuid;

use crate::config::Settings;
use crate::workers::media::image_io::load_image;
use crate::workers::media::media::{DecodedMedia, MediaFrame, MediaKind};

const POPPLER_REQUIRED: &str =
    "PDF support requires poppler-utils: pdfinfo, pdftoppm, and pdftotext on PATH";
const DOCUMENT_TEXT_LIMIT: usize = 64 * 1024;

#[derive(Clone, Debug)]
pub struct DecodedPdf {
    pub page_count: usize,
    pub indexed_page_count: usize,
    pub pages: Vec<DecodedPdfPage>,
    pub document_media: DecodedMedia,
    pub document_text: String,
}

#[derive(Clone, Debug)]
pub struct DecodedPdfPage {
    pub page_index: usize,
    pub page_number: usize,
    pub embedded_text: String,
    pub media: DecodedMedia,
}

pub fn is_pdf_extension(extension: &str) -> bool {
    extension.eq_ignore_ascii_case(".pdf")
}

pub fn is_pdf_content_type(content_type: &str) -> bool {
    content_type.eq_ignore_ascii_case("application/pdf")
}

pub fn pdf_upload_path(upload_dir: &Path, filename: Option<&str>) -> PathBuf {
    let extension = filename
        .and_then(|name| Path::new(name).extension())
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.is_empty())
        .map(|extension| format!(".{}", extension.to_ascii_lowercase()))
        .filter(|extension| is_pdf_extension(extension))
        .unwrap_or_else(|| ".pdf".to_string());
    upload_dir.join(format!("query-{}{extension}", Uuid::new_v4()))
}

pub fn write_pdf_upload(path: &Path, raw: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, raw).map_err(|error| error.to_string())
}

pub fn expose_source_pdf(
    path: &Path,
    pdf_id: &str,
    settings: &Settings,
) -> Result<Option<String>, String> {
    let output_dir = settings.upload_dir.join("source-pdfs");
    fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;
    let output_path = output_dir.join(format!("{pdf_id}.pdf"));
    if !output_path.exists() {
        match fs::hard_link(path, &output_path) {
            Ok(()) => {}
            Err(_) => {
                fs::copy(path, &output_path).map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(output_path
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| format!("/uploads/source-pdfs/{name}")))
}

pub fn decode_pdf(path: &Path, settings: &Settings) -> Result<DecodedPdf, String> {
    let page_count = pdf_page_count(path)?;
    if page_count == 0 {
        return Err("PDF does not contain any pages".to_string());
    }
    let indexed_page_count = page_count.min(settings.pdf_max_pages as usize);
    let mut pages = Vec::with_capacity(indexed_page_count);

    for page_number in 1..=indexed_page_count {
        let image = render_pdf_page(path, page_number, settings)?;
        let frame = MediaFrame {
            image: image.clone(),
            delay_ms: settings.gif_default_frame_delay_ms,
        };
        pages.push(DecodedPdfPage {
            page_index: page_number - 1,
            page_number,
            embedded_text: extract_pdf_page_text(path, page_number)?,
            media: DecodedMedia {
                kind: MediaKind::PdfPage,
                width: image.width(),
                height: image.height(),
                frame_count: Some(1),
                duration_ms: None,
                poster: image,
                sampled_frames: vec![frame.clone()],
                preview_frames: vec![frame],
                audio_analysis: None,
            },
        });
    }

    let summary_frames = sample_pdf_frames(&pages, settings.pdf_summary_pages);
    let poster = summary_frames
        .first()
        .map(|frame| frame.image.clone())
        .ok_or_else(|| "PDF did not produce any rendered page frames".to_string())?;
    let document_text = truncate_text(
        &pages
            .iter()
            .map(|page| page.embedded_text.as_str())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
        DOCUMENT_TEXT_LIMIT,
    );
    let document_media = DecodedMedia {
        kind: MediaKind::PdfDocument,
        width: poster.width(),
        height: poster.height(),
        frame_count: Some(summary_frames.len() as u32),
        duration_ms: None,
        poster,
        sampled_frames: summary_frames.clone(),
        preview_frames: summary_frames,
        audio_analysis: None,
    };

    Ok(DecodedPdf {
        page_count,
        indexed_page_count,
        pages,
        document_media,
        document_text,
    })
}

pub fn merge_pdf_text(embedded_text: &str, ocr_text: &str) -> String {
    let embedded = normalize_whitespace(embedded_text);
    let ocr = normalize_whitespace(ocr_text);
    if embedded.is_empty() {
        return ocr;
    }
    if ocr.is_empty() || embedded.to_lowercase().contains(&ocr.to_lowercase()) {
        embedded
    } else {
        format!("{embedded} {ocr}")
    }
}

fn pdf_page_count(path: &Path) -> Result<usize, String> {
    let output = run_command(Command::new("pdfinfo").arg(path))?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .find_map(|line| {
            line.strip_prefix("Pages:")
                .and_then(|value| value.trim().parse::<usize>().ok())
        })
        .ok_or_else(|| "Could not read PDF page count from pdfinfo output".to_string())
}

fn render_pdf_page(
    path: &Path,
    page_number: usize,
    settings: &Settings,
) -> Result<RgbImage, String> {
    fs::create_dir_all(&settings.upload_dir).map_err(|error| error.to_string())?;
    let prefix = settings
        .upload_dir
        .join(format!("pdf-page-{}-{page_number}", Uuid::new_v4()));
    let output_path = prefix.with_extension("png");
    let page = page_number.to_string();
    let dpi = settings.pdf_render_dpi.to_string();
    let result = run_command(
        Command::new("pdftoppm")
            .arg("-f")
            .arg(&page)
            .arg("-l")
            .arg(&page)
            .arg("-r")
            .arg(&dpi)
            .arg("-png")
            .arg("-singlefile")
            .arg(path)
            .arg(&prefix),
    )
    .and_then(|_| load_image(&output_path).map_err(|error| error.to_string()));
    let _ = fs::remove_file(&output_path);
    result
}

fn extract_pdf_page_text(path: &Path, page_number: usize) -> Result<String, String> {
    let page = page_number.to_string();
    let output = run_command(
        Command::new("pdftotext")
            .arg("-f")
            .arg(&page)
            .arg("-l")
            .arg(&page)
            .arg("-layout")
            .arg(path)
            .arg("-"),
    )?;
    Ok(normalize_whitespace(&String::from_utf8_lossy(
        &output.stdout,
    )))
}

fn run_command(command: &mut Command) -> Result<std::process::Output, String> {
    match command.output() {
        Ok(output) if output.status.success() => Ok(output),
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if stderr.is_empty() {
                Err(format!("PDF command failed with status {}", output.status))
            } else {
                Err(stderr)
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => Err(POPPLER_REQUIRED.to_string()),
        Err(error) => Err(error.to_string()),
    }
}

fn sample_pdf_frames(pages: &[DecodedPdfPage], limit: usize) -> Vec<MediaFrame> {
    if pages.len() <= limit {
        return pages
            .iter()
            .map(|page| page.media.sampled_frames[0].clone())
            .collect();
    }
    if limit == 1 {
        return vec![pages[0].media.sampled_frames[0].clone()];
    }
    let last = pages.len() - 1;
    let denominator = limit - 1;
    (0..limit)
        .map(|index| {
            let source_index = (index * last + denominator / 2) / denominator;
            pages[source_index].media.sampled_frames[0].clone()
        })
        .collect()
}

fn truncate_text(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_string()
}

#[cfg(test)]
mod tests {
    use super::{is_pdf_extension, merge_pdf_text, pdf_upload_path};

    #[test]
    fn pdf_extensions_match_case_insensitively() {
        assert!(is_pdf_extension(".pdf"));
        assert!(is_pdf_extension(".PDF"));
        assert!(!is_pdf_extension(".png"));
    }

    #[test]
    fn pdf_upload_path_uses_pdf_extension() {
        let path = pdf_upload_path(std::path::Path::new("/tmp/uploads"), Some("query.PDF"));
        assert_eq!(
            path.extension().and_then(|value| value.to_str()),
            Some("pdf")
        );
    }

    #[test]
    fn pdf_text_merge_deduplicates_ocr() {
        assert_eq!(merge_pdf_text("Invoice 123", "invoice 123"), "Invoice 123");
        assert_eq!(
            merge_pdf_text("Invoice 123", "Total due"),
            "Invoice 123 Total due"
        );
    }
}
