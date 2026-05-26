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
