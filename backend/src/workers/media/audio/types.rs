use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::thread;
use std::time::Duration;

use audio_analysis_core::FrameSpec;
use audio_analysis_rhythm::{
    detect_onsets, estimate_tempo, onset_envelope, Onset, OnsetDetectorConfig, TempoEstimatorConfig,
};
use audio_analysis_speakers::{
    EnergyVadConfig, EnergyVoiceActivityDetector, SpeakerAudio, VoiceActivityDetector,
};
use text_transcripts::{
    Transcriber, WhisperCppConfig, WhisperCppModel, WhisperCppModelStore, WhisperCppTranscriber,
};
use uuid::Uuid;

use crate::config::Settings;
use crate::domain::models::{
    AudioAnalysis, AudioRecognizedVoice, AudioSegmentGuess, AudioSpeechSegment,
    AudioTranscriptSegment,
};
use crate::workers::media::image_io::load_image;
use crate::workers::media::media::{DecodedMedia, MediaFrame, MediaKind};
use crate::workers::media::models::{audio_transcription_model_store, parse_whisper_cpp_model};
use crate::workers::media::voice::{VoiceRegistry, VoiceRegistryMatch};

const SPECTROGRAM_WIDTH: u32 = 512;
const SPECTROGRAM_HEIGHT: u32 = 256;
const AUDIO_ANALYSIS_SAMPLE_RATE: u32 = 16_000;
const AUDIO_ANALYSIS_MAX_SECONDS: f64 = 300.0;
const AUDIO_SEGMENT_MIN_SECONDS: f64 = 0.75;
const AUDIO_SEGMENT_MAX_SECONDS: f64 = 30.0;
const AUDIO_SEGMENT_ONSET_SPLIT_SECONDS: f64 = 8.0;
const TRANSCRIPTION_SAMPLE_RATE: u32 = 16_000;

#[derive(Clone, Debug)]
pub struct DecodedAudioSegment {
    pub scene_index: usize,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub speaker_id: Option<String>,
    pub speaker_label: Option<String>,
    pub media: DecodedMedia,
}

#[derive(Clone, Debug)]
pub struct SourceAudioSegment {
    pub scene_index: usize,
    pub start_seconds: f64,
    pub end_seconds: f64,
    pub full_audio_url: Option<String>,
    pub media: DecodedMedia,
}

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
    let duration_ms = audio_duration_ms(path).ok();
    let audio_analysis = analyze_audio(path, settings)?;
    decode_audio_window(path, settings, None, duration_ms, audio_analysis)
}

pub fn decode_audio_segments(
    path: &Path,
    settings: &Settings,
) -> Result<Vec<DecodedAudioSegment>, String> {
    decode_audio_segments_cancellable(path, settings, || false)
}

pub fn decode_audio_segments_cancellable(
    path: &Path,
    settings: &Settings,
    mut is_cancelled: impl FnMut() -> bool,
) -> Result<Vec<DecodedAudioSegment>, String> {
    check_cancelled(&mut is_cancelled)?;
    let duration_ms = audio_duration_ms_cancellable(path, &mut is_cancelled).ok();
    check_cancelled(&mut is_cancelled)?;
    let audio_analysis = analyze_audio_cancellable(path, settings, &mut is_cancelled)?;
    let windows = analysis_segment_windows(&audio_analysis, duration_ms);
    windows
        .into_iter()
        .enumerate()
        .map(|(scene_index, segment)| {
            check_cancelled(&mut is_cancelled)?;
            let media = decode_audio_window_cancellable(
                path,
                settings,
                Some((segment.start_seconds, segment.end_seconds)),
                Some(segment_duration_ms(&segment)),
                audio_analysis.clone(),
                &mut is_cancelled,
            )?;
            Ok(DecodedAudioSegment {
                scene_index,
                start_seconds: segment.start_seconds,
                end_seconds: segment.end_seconds,
                speaker_id: segment.speaker_id,
                speaker_label: segment.speaker_label,
                media,
            })
        })
        .collect()
}

pub fn decode_source_audio_segments(
    path: &Path,
    id_base: &str,
    settings: &Settings,
) -> Result<Vec<SourceAudioSegment>, String> {
    decode_source_audio_segments_cancellable(path, id_base, settings, || false)
}

pub fn decode_source_audio_segments_cancellable(
    path: &Path,
    id_base: &str,
    settings: &Settings,
    mut is_cancelled: impl FnMut() -> bool,
) -> Result<Vec<SourceAudioSegment>, String> {
    check_cancelled(&mut is_cancelled)?;
    let audio_id = crate::workers::media::image_io::image_id_for_uri(id_base);
    let full_audio_url = expose_source_audio(path, &audio_id, settings)?;
    check_cancelled(&mut is_cancelled)?;
    let segments = decode_audio_segments_cancellable(path, settings, &mut is_cancelled)?;
    Ok(segments
        .into_iter()
        .map(|segment| SourceAudioSegment {
            scene_index: segment.scene_index,
            start_seconds: segment.start_seconds,
            end_seconds: segment.end_seconds,
            full_audio_url: full_audio_url.clone(),
            media: segment.media,
        })
        .collect())
}

fn decode_audio_window(
    path: &Path,
    settings: &Settings,
    window: Option<(f64, f64)>,
    duration_ms: Option<u32>,
    audio_analysis: AudioAnalysis,
) -> Result<DecodedMedia, String> {
    decode_audio_window_cancellable(
        path,
        settings,
        window,
        duration_ms,
        audio_analysis,
        &mut || false,
    )
}

fn decode_audio_window_cancellable(
    path: &Path,
    settings: &Settings,
    window: Option<(f64, f64)>,
    duration_ms: Option<u32>,
    audio_analysis: AudioAnalysis,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<DecodedMedia, String> {
    let spectrogram_path = settings
        .upload_dir
        .join("audio-spectrograms")
        .join(format!("{}.png", Uuid::new_v4()));
    render_spectrogram_cancellable(path, &spectrogram_path, window, is_cancelled)?;
    check_cancelled(is_cancelled)?;
    let image = match load_image(&spectrogram_path) {
        Ok(image) => image,
        Err(error) => {
            let _ = fs::remove_file(&spectrogram_path);
            return Err(error.to_string());
        }
    };
    let _ = fs::remove_file(&spectrogram_path);

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
