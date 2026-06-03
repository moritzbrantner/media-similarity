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

fn run_command_output_cancellable(
    command: &mut Command,
    is_cancelled: &mut impl FnMut() -> bool,
) -> Result<Output, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(audio_tool_error)?;
    loop {
        check_cancelled(is_cancelled).inspect_err(|_| {
            let _ = child.kill();
            let _ = child.wait();
        })?;
        match child.try_wait().map_err(audio_tool_error)? {
            Some(_) => return child.wait_with_output().map_err(audio_tool_error),
            None => thread::sleep(Duration::from_millis(50)),
        }
    }
}

fn check_cancelled(is_cancelled: &mut impl FnMut() -> bool) -> Result<(), String> {
    if is_cancelled() {
        Err("job cancelled".to_string())
    } else {
        Ok(())
    }
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
        analyze_audio_samples, audio_upload_path, decode_source_audio_segments_cancellable,
        is_audio_content_type, is_audio_extension,
    };
    use crate::config::Settings;

    #[test]
    fn audio_detection_accepts_common_types_and_extensions() {
        assert!(is_audio_content_type("audio/mpeg"));
        assert!(is_audio_extension(".MP3"));
        assert!(is_audio_extension(".opus"));
        assert!(!is_audio_extension(".mp4"));
    }

    #[test]
    fn source_audio_decode_stops_before_opening_cancelled_work() {
        let settings = Settings::default();
        let error = decode_source_audio_segments_cancellable(
            std::path::Path::new("/does/not/exist.mp3"),
            "cancelled-audio",
            &settings,
            || true,
        )
        .unwrap_err();

        assert_eq!(error, "job cancelled");
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
