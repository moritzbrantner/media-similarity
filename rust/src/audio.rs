use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use audio_analysis_core::FrameSpec;
use audio_analysis_rhythm::{
    detect_onsets, estimate_tempo, onset_envelope, OnsetDetectorConfig, TempoEstimatorConfig,
};
use audio_analysis_speakers::{
    EnergyVadConfig, EnergyVoiceActivityDetector, SpeakerAudio, VoiceActivityDetector,
};
use uuid::Uuid;

use crate::config::Settings;
use crate::image_io::load_image;
use crate::media::{DecodedMedia, MediaFrame, MediaKind};
use crate::models::{AudioAnalysis, AudioSpeechSegment};

const SPECTROGRAM_WIDTH: u32 = 512;
const SPECTROGRAM_HEIGHT: u32 = 256;
const AUDIO_ANALYSIS_SAMPLE_RATE: u32 = 16_000;
const AUDIO_ANALYSIS_MAX_SECONDS: f64 = 300.0;

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
    let audio_analysis = analyze_audio(path)?;
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
        audio_analysis: Some(audio_analysis),
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

fn analyze_audio(path: &Path) -> Result<AudioAnalysis, String> {
    let samples = extract_mono_f32_samples(path, AUDIO_ANALYSIS_SAMPLE_RATE)?;
    analyze_audio_samples(&samples, AUDIO_ANALYSIS_SAMPLE_RATE).map_err(|error| error.to_string())
}

fn extract_mono_f32_samples(path: &Path, sample_rate: u32) -> Result<Vec<f32>, String> {
    let output = Command::new("ffmpeg")
        .arg("-nostdin")
        .arg("-v")
        .arg("error")
        .arg("-i")
        .arg(path)
        .arg("-map")
        .arg("0:a:0")
        .arg("-t")
        .arg(format!("{AUDIO_ANALYSIS_MAX_SECONDS:.3}"))
        .arg("-vn")
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg(sample_rate.to_string())
        .arg("-f")
        .arg("f32le")
        .arg("pipe:1")
        .output()
        .map_err(audio_tool_error)?;
    if !output.status.success() {
        return Err(command_error("ffmpeg", &output.stderr));
    }
    if output.stdout.len() % std::mem::size_of::<f32>() != 0 {
        return Err("ffmpeg returned incomplete f32 audio samples".to_string());
    }
    Ok(output
        .stdout
        .chunks_exact(std::mem::size_of::<f32>())
        .map(|chunk| f32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]))
        .filter(|sample| sample.is_finite())
        .collect())
}

fn analyze_audio_samples(
    samples: &[f32],
    sample_rate: u32,
) -> video_analysis_core::Result<AudioAnalysis> {
    if sample_rate == 0 || samples.is_empty() {
        return Ok(AudioAnalysis {
            speech_detected: false,
            speech_ratio: 0.0,
            speech_segments: Vec::new(),
            tempo_bpm: None,
            tempo_confidence: 0.0,
            tempo_onset_count: 0,
        });
    }

    let audio = SpeakerAudio::mono(samples, sample_rate)?;
    let mut vad = EnergyVoiceActivityDetector::new(EnergyVadConfig {
        rms_threshold: 0.01,
        frame_seconds: 0.03,
        hop_seconds: 0.01,
        min_speech_seconds: 0.12,
        merge_gap_seconds: 0.08,
    })?;
    let speech_segments = vad.detect_speech(&audio)?;
    let analyzed_seconds = samples.len() as f64 / sample_rate as f64;
    let speech_seconds = speech_segments
        .iter()
        .map(|span| span.duration_seconds())
        .sum::<f64>();
    let speech_ratio = if analyzed_seconds > f64::EPSILON {
        (speech_seconds / analyzed_seconds).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };

    let frame_size = (sample_rate as usize / 20).max(1);
    let hop_size = (sample_rate as usize / 100).max(1);
    let frame_spec = FrameSpec::new(frame_size, hop_size)?;
    let envelope = onset_envelope(samples, sample_rate, frame_spec)?;
    let onsets = detect_onsets(
        &envelope,
        OnsetDetectorConfig {
            strength_threshold: 0.03,
            min_interval_seconds: 0.08,
        },
    )?;
    let tempo = estimate_tempo(&onsets, TempoEstimatorConfig::default())?;

    Ok(AudioAnalysis {
        speech_detected: !speech_segments.is_empty(),
        speech_ratio,
        speech_segments: speech_segments
            .into_iter()
            .map(|span| AudioSpeechSegment {
                start_seconds: span.start_seconds,
                end_seconds: span.end_seconds,
                confidence: span.score.clamp(0.0, 1.0),
            })
            .collect(),
        tempo_bpm: tempo.bpm.map(|bpm| (bpm * 10.0).round() / 10.0),
        tempo_confidence: tempo.confidence.clamp(0.0, 1.0),
        tempo_onset_count: tempo.onset_count as u32,
    })
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
    use super::{
        analyze_audio_samples, audio_upload_path, is_audio_content_type, is_audio_extension,
    };

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

    #[test]
    fn audio_analysis_detects_speech_activity() {
        let sample_rate = 2_000;
        let samples = (0..sample_rate)
            .map(|index| {
                let phase = index as f32 * 2.0 * std::f32::consts::PI * 220.0 / sample_rate as f32;
                phase.sin() * 0.2
            })
            .collect::<Vec<_>>();
        let analysis = analyze_audio_samples(&samples, sample_rate).unwrap();
        assert!(analysis.speech_detected);
        assert!(analysis.speech_ratio > 0.5);
    }

    #[test]
    fn audio_analysis_estimates_click_track_tempo() {
        let sample_rate = 2_000;
        let seconds = 5;
        let bpm = 120.0;
        let interval = (sample_rate as f32 * 60.0 / bpm) as usize;
        let mut samples = vec![0.0; sample_rate as usize * seconds];
        for start in (0..samples.len()).step_by(interval) {
            for sample in samples.iter_mut().skip(start).take(16) {
                *sample = 1.0;
            }
        }

        let analysis = analyze_audio_samples(&samples, sample_rate).unwrap();
        let detected = analysis.tempo_bpm.unwrap();
        assert!(
            (detected - bpm).abs() <= 2.0,
            "expected {bpm}, got {detected}"
        );
        assert!(analysis.tempo_confidence > 0.5);
    }

    #[test]
    fn audio_analysis_handles_silence() {
        let analysis = analyze_audio_samples(&vec![0.0; 2_000], 2_000).unwrap();
        assert!(!analysis.speech_detected);
        assert_eq!(analysis.speech_ratio, 0.0);
        assert_eq!(analysis.tempo_bpm, None);
    }
}
