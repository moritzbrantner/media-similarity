use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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
    let duration_ms = audio_duration_ms(path).ok();
    let audio_analysis = analyze_audio(path, settings)?;
    let windows = analysis_segment_windows(&audio_analysis, duration_ms);
    windows
        .into_iter()
        .enumerate()
        .map(|(scene_index, segment)| {
            let media = decode_audio_window(
                path,
                settings,
                Some((segment.start_seconds, segment.end_seconds)),
                Some(segment_duration_ms(&segment)),
                audio_analysis.clone(),
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
    let audio_id = crate::workers::media::image_io::image_id_for_uri(id_base);
    let full_audio_url = expose_source_audio(path, &audio_id, settings)?;
    let segments = decode_audio_segments(path, settings)?;
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
    let spectrogram_path = settings
        .upload_dir
        .join("audio-spectrograms")
        .join(format!("{}.png", Uuid::new_v4()));
    render_spectrogram(path, &spectrogram_path, window)?;
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

fn render_spectrogram(
    input_path: &Path,
    output_path: &Path,
    window: Option<(f64, f64)>,
) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let filter = format!(
        "showspectrumpic=s={}x{}:legend=disabled:scale=log",
        SPECTROGRAM_WIDTH, SPECTROGRAM_HEIGHT
    );
    let mut command = Command::new("ffmpeg");
    command.arg("-y").arg("-v").arg("error");
    if let Some((start_seconds, end_seconds)) = window {
        command
            .arg("-ss")
            .arg(format!("{:.3}", start_seconds.max(0.0)))
            .arg("-t")
            .arg(format!(
                "{:.3}",
                (end_seconds - start_seconds).max(AUDIO_SEGMENT_MIN_SECONDS)
            ));
    }
    let output = command
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

fn analyze_audio(path: &Path, settings: &Settings) -> Result<AudioAnalysis, String> {
    let samples = extract_mono_f32_samples(path, AUDIO_ANALYSIS_SAMPLE_RATE)?;
    let mut analysis = analyze_audio_samples(&samples, AUDIO_ANALYSIS_SAMPLE_RATE)
        .map_err(|error| error.to_string())?;
    attach_voice_registry(
        &mut analysis,
        &samples,
        AUDIO_ANALYSIS_SAMPLE_RATE,
        settings,
    )?;
    attach_audio_transcription(&mut analysis, path, settings);
    Ok(analysis)
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
            audio_segments: Vec::new(),
            recognized_voices: Vec::new(),
            transcript_text: String::new(),
            transcript_language: None,
            transcript_segments: Vec::new(),
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
    let speech_segments = speech_segments
        .into_iter()
        .map(|span| AudioSpeechSegment {
            start_seconds: span.start_seconds,
            end_seconds: span.end_seconds,
            confidence: span.score.clamp(0.0, 1.0),
        })
        .collect::<Vec<_>>();
    let audio_segments = guess_audio_segments(analyzed_seconds, &speech_segments, &onsets);

    Ok(AudioAnalysis {
        speech_detected: !speech_segments.is_empty(),
        speech_ratio,
        speech_segments,
        audio_segments,
        recognized_voices: Vec::new(),
        transcript_text: String::new(),
        transcript_language: None,
        transcript_segments: Vec::new(),
        tempo_bpm: tempo.bpm.map(|bpm| (bpm * 10.0).round() / 10.0),
        tempo_confidence: tempo.confidence.clamp(0.0, 1.0),
        tempo_onset_count: tempo.onset_count as u32,
    })
}

fn attach_audio_transcription(analysis: &mut AudioAnalysis, path: &Path, settings: &Settings) {
    if !settings.audio_transcription_enabled || !analysis.speech_detected {
        return;
    }
    match transcribe_audio(path, settings) {
        Ok(Some(transcript)) => {
            analysis.transcript_text = transcript.text;
            analysis.transcript_language = transcript.language;
            analysis.transcript_segments = transcript.segments;
        }
        Ok(None) => {}
        Err(error) => {
            tracing::warn!(%error, path = %path.display(), "Audio transcription failed");
        }
    }
}

#[derive(Debug)]
struct AudioTranscript {
    text: String,
    language: Option<String>,
    segments: Vec<AudioTranscriptSegment>,
}

fn transcribe_audio(path: &Path, settings: &Settings) -> Result<Option<AudioTranscript>, String> {
    let model = parse_whisper_cpp_model(&settings.audio_transcription_model)?;
    let store = audio_transcription_model_store(settings);
    if !settings.audio_transcription_auto_download && !whisper_model_is_cached(&store, model) {
        tracing::warn!(
            model = model.id(),
            "Skipping audio transcription because the whisper.cpp model is not cached"
        );
        return Ok(None);
    }

    let wav_path = transcription_wav_path(settings);
    transcode_for_transcription(path, &wav_path)?;
    let mut transcriber = WhisperCppTranscriber::new(WhisperCppConfig {
        model,
        language: settings.audio_transcription_language.clone(),
        translate: false,
        threads: settings.audio_transcription_threads,
    })
    .with_model_store(store);
    let result = transcriber
        .transcribe(&wav_path)
        .map_err(|error| error.to_string());
    let _ = fs::remove_file(&wav_path);
    let result = result?;
    let text = result
        .text
        .unwrap_or_else(|| {
            result
                .segments
                .iter()
                .map(|segment| segment.text.trim())
                .filter(|text| !text.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
        })
        .trim()
        .to_string();
    if text.is_empty() {
        return Ok(None);
    }
    Ok(Some(AudioTranscript {
        text,
        language: result.language,
        segments: result
            .segments
            .into_iter()
            .filter_map(|segment| {
                let text = segment.text.trim().to_string();
                (!text.is_empty()).then_some(AudioTranscriptSegment {
                    segment_index: segment.index,
                    start_seconds: segment.start_seconds,
                    end_seconds: segment.end_seconds,
                    text,
                    confidence: segment.confidence,
                })
            })
            .collect(),
    }))
}

fn transcription_wav_path(settings: &Settings) -> PathBuf {
    settings
        .upload_dir
        .join("audio-transcription")
        .join(format!("{}.wav", Uuid::new_v4()))
}

fn transcode_for_transcription(input_path: &Path, output_path: &Path) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let output = Command::new("ffmpeg")
        .arg("-y")
        .arg("-nostdin")
        .arg("-v")
        .arg("error")
        .arg("-i")
        .arg(input_path)
        .arg("-map")
        .arg("0:a:0")
        .arg("-t")
        .arg(format!("{AUDIO_ANALYSIS_MAX_SECONDS:.3}"))
        .arg("-vn")
        .arg("-ac")
        .arg("1")
        .arg("-ar")
        .arg(TRANSCRIPTION_SAMPLE_RATE.to_string())
        .arg("-f")
        .arg("wav")
        .arg(output_path)
        .output()
        .map_err(audio_tool_error)?;
    if output.status.success() {
        return Ok(());
    }
    Err(command_error("ffmpeg", &output.stderr))
}

pub fn whisper_model_is_cached(store: &WhisperCppModelStore, model: WhisperCppModel) -> bool {
    store
        .catalog()
        .models
        .into_iter()
        .any(|status| status.model == model && status.cached)
}

fn attach_voice_registry(
    analysis: &mut AudioAnalysis,
    samples: &[f32],
    sample_rate: u32,
    settings: &Settings,
) -> Result<(), String> {
    if samples.is_empty() || analysis.audio_segments.is_empty() {
        return Ok(());
    }

    let mut registry = VoiceRegistry::load(settings)?;
    let mut by_voice = std::collections::BTreeMap::<String, AudioRecognizedVoiceAccumulator>::new();
    for segment in analysis
        .audio_segments
        .iter_mut()
        .filter(|segment| segment.kind == "speech")
    {
        let start = (segment.start_seconds * sample_rate as f64).round() as usize;
        let end = (segment.end_seconds * sample_rate as f64).round() as usize;
        if end <= start
            || end.min(samples.len()) - start.min(samples.len()) < sample_rate as usize / 4
        {
            continue;
        }
        let VoiceRegistryMatch {
            id,
            label,
            score,
            confidence: _,
        } = registry.recognize_or_enroll(
            &samples[start.min(samples.len())..end.min(samples.len())],
            sample_rate,
        )?;
        segment.speaker_id = Some(id.clone());
        segment.speaker_label = Some(label.clone());
        let entry = by_voice
            .entry(id.clone())
            .or_insert_with(|| AudioRecognizedVoiceAccumulator {
                id,
                label,
                segment_count: 0,
                total_seconds: 0.0,
                score_sum: 0.0,
            });
        entry.segment_count += 1;
        entry.total_seconds += segment.end_seconds - segment.start_seconds;
        entry.score_sum += score;
    }
    registry.save_if_changed()?;
    analysis.recognized_voices = by_voice
        .into_values()
        .map(|value| AudioRecognizedVoice {
            id: value.id,
            label: value.label,
            segment_count: value.segment_count,
            total_seconds: (value.total_seconds * 1000.0).round() / 1000.0,
            confidence: if value.segment_count == 0 {
                0.0
            } else {
                (value.score_sum / value.segment_count as f32).clamp(0.0, 1.0)
            },
        })
        .collect();
    Ok(())
}

#[derive(Debug)]
struct AudioRecognizedVoiceAccumulator {
    id: String,
    label: String,
    segment_count: u32,
    total_seconds: f64,
    score_sum: f32,
}

fn analysis_segment_windows(
    analysis: &AudioAnalysis,
    duration_ms: Option<u32>,
) -> Vec<AudioSegmentGuess> {
    if !analysis.audio_segments.is_empty() {
        return analysis.audio_segments.clone();
    }
    let duration_seconds = duration_ms
        .map(|value| value as f64 / 1000.0)
        .unwrap_or(0.0);
    if duration_seconds > 0.0 {
        vec![AudioSegmentGuess {
            segment_index: 0,
            kind: "audio".to_string(),
            start_seconds: 0.0,
            end_seconds: duration_seconds,
            confidence: 0.25,
            speaker_id: None,
            speaker_label: None,
        }]
    } else {
        Vec::new()
    }
}

fn guess_audio_segments(
    duration_seconds: f64,
    speech_segments: &[AudioSpeechSegment],
    onsets: &[Onset],
) -> Vec<AudioSegmentGuess> {
    if duration_seconds <= 0.0 || !duration_seconds.is_finite() {
        return Vec::new();
    }

    let mut boundaries = vec![0.0, duration_seconds];
    for segment in speech_segments {
        boundaries.push(segment.start_seconds.clamp(0.0, duration_seconds));
        boundaries.push(segment.end_seconds.clamp(0.0, duration_seconds));
    }

    let mut last_onset_boundary = 0.0;
    for onset in onsets {
        if onset.timestamp_seconds <= 0.0 || onset.timestamp_seconds >= duration_seconds {
            continue;
        }
        if onset.timestamp_seconds - last_onset_boundary >= AUDIO_SEGMENT_ONSET_SPLIT_SECONDS {
            boundaries.push(onset.timestamp_seconds);
            last_onset_boundary = onset.timestamp_seconds;
        }
    }

    boundaries.sort_by(f64::total_cmp);
    boundaries.dedup_by(|left, right| (*left - *right).abs() < 0.25);
    let mut expanded = Vec::new();
    for pair in boundaries.windows(2) {
        let mut start = pair[0];
        let end = pair[1];
        while end - start > AUDIO_SEGMENT_MAX_SECONDS {
            expanded.push(start);
            start += AUDIO_SEGMENT_MAX_SECONDS;
        }
        expanded.push(start);
        expanded.push(end);
    }
    expanded.sort_by(f64::total_cmp);
    expanded.dedup_by(|left, right| (*left - *right).abs() < 0.25);

    let mut segments = Vec::new();
    for pair in expanded.windows(2) {
        let start = pair[0].max(0.0);
        let end = pair[1].min(duration_seconds);
        if end - start < AUDIO_SEGMENT_MIN_SECONDS {
            continue;
        }
        let speech_overlap = speech_overlap_seconds(start, end, speech_segments);
        let onset_count = onsets
            .iter()
            .filter(|onset| onset.timestamp_seconds >= start && onset.timestamp_seconds < end)
            .count();
        if speech_overlap <= 0.0 && onset_count == 0 && !speech_segments.is_empty() {
            continue;
        }
        let kind = if speech_overlap / (end - start) >= 0.35 {
            "speech"
        } else if onset_count > 0 {
            "music_or_sound"
        } else {
            "audio"
        };
        let confidence = if kind == "speech" {
            (speech_overlap / (end - start)).clamp(0.0, 1.0) as f32
        } else if onset_count > 0 {
            (onset_count as f32 / 6.0).clamp(0.25, 0.85)
        } else {
            0.25
        };
        segments.push(AudioSegmentGuess {
            segment_index: segments.len(),
            kind: kind.to_string(),
            start_seconds: (start * 1000.0).round() / 1000.0,
            end_seconds: (end * 1000.0).round() / 1000.0,
            confidence,
            speaker_id: None,
            speaker_label: None,
        });
    }

    if segments.is_empty() {
        segments.push(AudioSegmentGuess {
            segment_index: 0,
            kind: "audio".to_string(),
            start_seconds: 0.0,
            end_seconds: (duration_seconds * 1000.0).round() / 1000.0,
            confidence: 0.25,
            speaker_id: None,
            speaker_label: None,
        });
    }
    segments
}

fn speech_overlap_seconds(
    start_seconds: f64,
    end_seconds: f64,
    speech_segments: &[AudioSpeechSegment],
) -> f64 {
    speech_segments
        .iter()
        .map(|segment| {
            (segment.end_seconds.min(end_seconds) - segment.start_seconds.max(start_seconds))
                .max(0.0)
        })
        .sum()
}

fn segment_duration_ms(segment: &AudioSegmentGuess) -> u32 {
    ((segment.end_seconds - segment.start_seconds).max(0.001) * 1000.0).round() as u32
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
