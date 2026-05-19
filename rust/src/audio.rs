use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use uuid::Uuid;

use crate::config::Settings;
use crate::image_io::load_image;
use crate::media::{DecodedMedia, MediaFrame, MediaKind};

const SPECTROGRAM_WIDTH: u32 = 512;
const SPECTROGRAM_HEIGHT: u32 = 256;

pub fn is_audio_extension(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        ".mp3" | ".wav" | ".flac" | ".m4a" | ".aac" | ".ogg" | ".opus" | ".wma" | ".aiff" | ".aif"
    )
}

pub fn is_audio_content_type(content_type: &str) -> bool {
    content_type.to_ascii_lowercase().starts_with("audio/")
}

pub fn audio_upload_path(upload_dir: &Path, filename: Option<&str>) -> PathBuf {
    let extension = filename
        .and_then(|name| Path::new(name).extension())
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.is_empty())
        .map(|extension| format!(".{}", extension.to_ascii_lowercase()))
        .filter(|extension| is_audio_extension(extension))
        .unwrap_or_else(|| ".mp3".to_string());
    upload_dir.join(format!("query-{}{extension}", Uuid::new_v4()))
}

pub fn write_audio_upload(path: &Path, raw: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, raw).map_err(|error| error.to_string())
}

pub fn decode_audio(path: &Path, settings: &Settings) -> Result<DecodedMedia, String> {
    let spectrogram_path = settings
        .upload_dir
        .join("audio-spectrograms")
        .join(format!("{}.png", Uuid::new_v4()));
    render_spectrogram(path, &spectrogram_path)?;
    let image = match load_image(&spectrogram_path) {
        Ok(image) => image,
        Err(error) => {
            let _ = fs::remove_file(&spectrogram_path);
            return Err(error.to_string());
        }
    };
    let _ = fs::remove_file(&spectrogram_path);

    let duration_ms = audio_duration_ms(path).ok();
    let frame = MediaFrame {
        image: image.clone(),
        delay_ms: duration_ms
            .unwrap_or(settings.gif_default_frame_delay_ms)
            .max(1),
    };

    Ok(DecodedMedia {
        kind: MediaKind::Audio,
        width: image.width(),
        height: image.height(),
        frame_count: None,
        duration_ms,
        poster: image,
        sampled_frames: vec![frame.clone()],
        preview_frames: vec![frame],
    })
}

pub fn expose_source_audio(
    path: &Path,
    audio_id: &str,
    settings: &Settings,
) -> Result<Option<String>, String> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .filter(|extension| !extension.is_empty())
        .unwrap_or("mp3");
    let output_dir = settings.upload_dir.join("source-audio");
    fs::create_dir_all(&output_dir).map_err(|error| error.to_string())?;
    let output_path = output_dir.join(format!("{audio_id}.{extension}"));
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
        .map(|name| format!("/uploads/source-audio/{name}")))
}

fn render_spectrogram(input_path: &Path, output_path: &Path) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let filter = format!(
        "showspectrumpic=s={}x{}:legend=disabled:scale=log",
        SPECTROGRAM_WIDTH, SPECTROGRAM_HEIGHT
    );
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-v")
        .arg("error")
        .arg("-i")
        .arg(input_path)
        .arg("-lavfi")
        .arg(filter)
        .arg("-frames:v")
        .arg("1")
        .arg(output_path)
        .output()
        .map_err(audio_tool_error)?;
    if output.status.success() {
        return Ok(());
    }

    Err(command_error("ffmpeg", &output.stderr))
}

fn audio_duration_ms(path: &Path) -> Result<u32, String> {
    let output = Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("format=duration")
        .arg("-of")
        .arg("default=noprint_wrappers=1:nokey=1")
        .arg(path)
        .output()
        .map_err(audio_tool_error)?;
    if !output.status.success() {
        return Err(command_error("ffprobe", &output.stderr));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let seconds = text
        .trim()
        .parse::<f64>()
        .map_err(|_| "ffprobe did not return a numeric duration".to_string())?;
    Ok((seconds.max(0.0) * 1000.0).round() as u32)
}

fn command_error(program: &str, stderr: &[u8]) -> String {
    let message = String::from_utf8_lossy(stderr).trim().to_string();
    if message.is_empty() {
        format!("{program} failed while processing audio")
    } else {
        message
    }
}

fn audio_tool_error(error: impl std::fmt::Display) -> String {
    let message = error.to_string();
    if message.contains("No such file or directory") || message.contains("failed to start") {
        format!("{message}. Audio support requires ffmpeg and ffprobe on PATH")
    } else {
        message
    }
}

#[cfg(test)]
mod tests {
    use super::{audio_upload_path, is_audio_content_type, is_audio_extension};

    #[test]
    fn audio_detection_accepts_common_types_and_extensions() {
        assert!(is_audio_content_type("audio/mpeg"));
        assert!(is_audio_extension(".MP3"));
        assert!(is_audio_extension(".opus"));
        assert!(!is_audio_extension(".mp4"));
    }

    #[test]
    fn audio_upload_path_keeps_supported_extension() {
        let path = audio_upload_path(std::path::Path::new("/tmp/uploads"), Some("song.FLAC"));
        assert_eq!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("flac")
        );
    }
}
