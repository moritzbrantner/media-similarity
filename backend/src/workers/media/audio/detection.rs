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
