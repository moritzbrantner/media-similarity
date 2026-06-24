fn transcribe_audio(path: &Path, settings: &Settings) -> Result<AudioTranscript, String> {
    let bundle = load_role_bundle(ModelRole::AudioTranscription, settings)?;
    let wav_path = transcription_wav_path(settings);
    transcode_for_transcription(path, &wav_path)?;
    let request = native_transcription_request(&wav_path, &bundle.root, settings)?;
    let result = audio_analysis_transcription::transcribe(request).map_err(|error| {
        format!(
            "native audio transcription failed for `{}`: {error}",
            path.display()
        )
    });
    let _ = fs::remove_file(&wav_path);
    audio_transcript_from_native(result?.transcript)
}

fn native_transcription_request(
    wav_path: &Path,
    bundle_path: &Path,
    settings: &Settings,
) -> Result<TranscriptionPipelineRequest, String> {
    Ok(TranscriptionPipelineRequest {
        source: TranscriptionSource::Path {
            path: wav_path.to_path_buf(),
        },
        provider: TranscriptionProviderSelection::CandleWhisper(CandleWhisperOptions {
            model_id: settings.audio_transcription_model.clone(),
            language: settings.audio_transcription_language.clone(),
            device: native_device_preference(&settings.audio_transcription_device)?,
            model_bundle: Some(bundle_path.to_path_buf()),
            model_cache_only: !settings.audio_transcription_auto_download,
            batch_chunks: settings.audio_transcription_batch_chunks,
            max_batch_size: settings.audio_transcription_max_batch_size,
            ..CandleWhisperOptions::default()
        }),
        vad: audio_analysis_transcription::VadOptions {
            enabled: false,
            ..audio_analysis_transcription::VadOptions::default()
        },
        alignment: audio_analysis_transcription::AlignmentOptions::default(),
        diarization: audio_analysis_transcription::DiarizationOptions::default(),
        output: audio_analysis_transcription::TranscriptionOutputOptions::default(),
    })
}

fn audio_transcript_from_native(
    transcript: impl serde::Serialize,
) -> Result<AudioTranscript, String> {
    let transcript: text_transcripts::TranscriptionContract =
        serde_json::from_value(serde_json::to_value(transcript).map_err(|error| {
            format!("native audio transcription returned an unsupported transcript: {error}")
        })?)
        .map_err(|error| {
            format!("native audio transcription returned an invalid transcript: {error}")
        })?;
    let text = transcript
        .text
        .unwrap_or_else(|| {
            transcript
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
        return Err("native audio transcription produced an empty transcript".to_string());
    }
    Ok(AudioTranscript {
        text,
        language: transcript.language.clone(),
        segments: transcript
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
    })
}

fn native_device_preference(value: &str) -> Result<NativeDevicePreference, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(NativeDevicePreference::Auto),
        "cpu" => Ok(NativeDevicePreference::Cpu),
        "cuda" => Ok(NativeDevicePreference::Cuda),
        other => Err(format!(
            "Unsupported native audio transcription device `{other}`"
        )),
    }
}

fn transcription_wav_path(settings: &Settings) -> PathBuf {
    settings
        .upload_dir
        .join("audio-transcription")
        .join(format!("{}.wav", Uuid::new_v4()))
}

fn transcode_for_transcription(input_path: &Path, output_path: &Path) -> Result<(), String> {
    transcode_for_transcription_cancellable(input_path, output_path, &mut || false)
}

fn transcode_for_transcription_cancellable(
    input_path: &Path,
    output_path: &Path,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let mut command = Command::new("ffmpeg");
    command
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
        .arg(output_path);
    let output = run_command_output_cancellable(&mut command, is_cancelled)?;
    if output.status.success() {
        return Ok(());
    }
    Err(command_error("ffmpeg", &output.stderr))
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

#[cfg(test)]
mod transcription_tests {
    use super::{
        audio_transcript_from_native, native_transcription_request, AudioTranscriptSegment,
    };
    use audio_analysis_transcription::{
        NativeDevicePreference, TranscriptionProviderSelection, TranscriptionSource,
    };
    use serde_json::json;

    use crate::config::Settings;

    #[test]
    fn native_transcription_request_uses_candle_bundle_and_cuda_preference() {
        let settings = Settings {
            audio_transcription_device: "cuda".to_string(),
            audio_transcription_model: "openai/whisper-large-v3-turbo".to_string(),
            audio_transcription_language: Some("en".to_string()),
            audio_transcription_auto_download: false,
            audio_transcription_batch_chunks: false,
            audio_transcription_max_batch_size: Some(2),
            ..Settings::default()
        };

        let request = native_transcription_request(
            std::path::Path::new("/tmp/audio.wav"),
            std::path::Path::new("/models/whisper-large-v3-turbo"),
            &settings,
        )
        .unwrap();

        assert_eq!(
            request.source,
            TranscriptionSource::Path {
                path: "/tmp/audio.wav".into()
            }
        );
        let TranscriptionProviderSelection::CandleWhisper(options) = request.provider else {
            panic!("expected native Candle Whisper provider");
        };
        assert_eq!(options.model_id, "openai/whisper-large-v3-turbo");
        assert_eq!(options.language.as_deref(), Some("en"));
        assert_eq!(options.device, NativeDevicePreference::Cuda);
        assert_eq!(
            options.model_bundle.as_deref(),
            Some(std::path::Path::new("/models/whisper-large-v3-turbo"))
        );
        assert!(options.model_cache_only);
        assert!(!options.batch_chunks);
        assert_eq!(options.max_batch_size, Some(2));
        assert!(!request.vad.enabled);
    }

    #[test]
    fn native_transcript_mapping_joins_segments_and_preserves_timing() {
        let transcript = audio_transcript_from_native(json!({
            "language": "en",
            "segments": [
                {
                    "index": 7,
                    "startSeconds": 1.25,
                    "endSeconds": 2.5,
                    "text": " first phrase ",
                    "confidence": 0.8,
                    "isFinal": true
                },
                {
                    "index": 8,
                    "startSeconds": 2.5,
                    "endSeconds": 3.75,
                    "text": "second phrase",
                    "isFinal": true
                }
            ]
        }))
        .unwrap();

        assert_eq!(transcript.text, "first phrase second phrase");
        assert_eq!(transcript.language.as_deref(), Some("en"));
        assert_eq!(
            transcript.segments,
            vec![
                AudioTranscriptSegment {
                    segment_index: 7,
                    start_seconds: Some(1.25),
                    end_seconds: Some(2.5),
                    text: "first phrase".to_string(),
                    confidence: Some(0.8),
                },
                AudioTranscriptSegment {
                    segment_index: 8,
                    start_seconds: Some(2.5),
                    end_seconds: Some(3.75),
                    text: "second phrase".to_string(),
                    confidence: None,
                },
            ]
        );
    }

    #[test]
    fn native_transcript_mapping_fails_on_empty_transcript() {
        let error = audio_transcript_from_native(json!({
            "text": "   ",
            "segments": [],
        }))
        .unwrap_err();

        assert_eq!(
            error,
            "native audio transcription produced an empty transcript"
        );
    }
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
