//! Translate the public composition facade into application-owned video metadata.
use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::{Path, PathBuf};

use media_intelligence_analysis::visible_text::{
    OcrBackendConfig, VisualAnalysisOcrConfig, VisualAnalysisOcrProducer,
};
use media_intelligence_analysis::{
    analyze_with_exact_scene_timing, AnalysisProfile, AnalysisRequest, AssetId, CancellationToken,
    ContentHash, InMemoryStageCache, MediaIdentity, Modality, ProducerError, ProducerIdentity,
    ProducerInput, ProducerReceipt, Producers, SceneConfig, SpeechProducer, SpeechRun, StageStatus,
};
use media_intelligence_core::exact_timing::{
    ExactAnchorTimingV1, ExactAnchorV1, EXACT_TIMING_SCHEMA, EXACT_TIMING_SCHEMA_VERSION,
};
use media_intelligence_core::{
    EvidenceContent, EvidenceItem, RelationKind, SceneEvidence, TranscriptSegment,
};
use sha2::{Digest, Sha256};
use video_analysis_core::{FramePosition, Scene, Timebase, Timestamp};

use crate::config::Settings;
use crate::domain::models::{
    AudioAnalysis, EvidenceProducer, EvidenceStatus, OcrAnalysis, OcrFrameText, VideoEvidence,
    VideoEvidenceRelation, VideoSpeechEvidence, VideoTextEvidence, VideoTextObservation,
};

pub struct AnalyzedVideoScene {
    pub scene_index: usize,
    pub scene: Scene,
    pub ocr: OcrAnalysis,
    pub evidence: VideoEvidence,
}

/// Analyse each source once. The existing audio adapter has already transcribed it;
/// its timed segments enter the shared composition without another ASR invocation.
pub fn analyze_video(
    path: &Path,
    id_base: &str,
    audio: Option<&AudioAnalysis>,
    settings: &Settings,
) -> Result<Vec<AnalyzedVideoScene>, String> {
    let hash = content_hash(path)?;
    let asset_id = crate::workers::media::image_io::image_id_for_uri(id_base);
    let request = AnalysisRequest {
        media: MediaIdentity {
            asset_id: AssetId::new(asset_id.clone()).map_err(|error| error.to_string())?,
            content_hash: ContentHash::parse(hash.clone()).map_err(|error| error.to_string())?,
            duration_seconds: None,
        },
        media_path: path.to_owned(),
        profile: AnalysisProfile {
            speech: audio.is_some(),
            visible_text: settings.ocr_enabled,
            scenes: Some(SceneConfig::default()),
        },
    };
    let mut config = VisualAnalysisOcrConfig::new(OcrBackendConfig::JsonCommand {
        name: format!("tesseract:{}", settings.ocr_command),
        command: bridge_executable()?,
        args: vec![
            "--video-ocr-json".into(),
            settings.ocr_command.clone(),
            settings.ocr_language.clone().unwrap_or_default(),
            "{image}".into(),
        ],
    });
    config.model_version = std::process::Command::new(&settings.ocr_command)
        .arg("--version")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|output| output.lines().next().map(str::to_owned));
    let ocr = VisualAnalysisOcrProducer::new(config).map_err(|error| error.to_string())?;
    let speech = ExistingSpeech { audio, settings };
    let analyzed = analyze_with_exact_scene_timing(
        &request,
        &Producers {
            speech: audio.map(|_| &speech as &dyn SpeechProducer),
            visible_text: settings.ocr_enabled.then_some(&ocr),
        },
        &mut InMemoryStageCache::new(),
        &CancellationToken::new(),
    )
    .map_err(|error| error.to_string())?;
    let report = &analyzed.legacy;
    if let StageStatus::Failed { reason, .. } = &report.scenes {
        return Err(reason.clone());
    }
    // The opt-in producer is the only authority for source PTS. Index anchors
    // once by producer-local scene id, never infer them from v1 floating seconds.
    let exact_scene_anchors = analyzed
        .exact_timing
        .as_ref()
        .map(
            |sidecar| -> Result<BTreeMap<&str, &ExactAnchorV1>, String> {
                if sidecar.schema != EXACT_TIMING_SCHEMA
                    || sidecar.schema_version != EXACT_TIMING_SCHEMA_VERSION
                    || sidecar.clock != "media_relative"
                    || sidecar.media != request.media
                    || !sidecar.exact_modalities.contains(&Modality::Scene)
                {
                    return Err(
                        "exact timing sidecar has invalid media or source clock identity".into(),
                    );
                }
                let mut anchors = BTreeMap::new();
                for anchor in &sidecar.anchors {
                    if anchor.receipt.modality == Modality::Scene
                        && anchors.insert(anchor.record_id.as_str(), anchor).is_some()
                    {
                        return Err("duplicate exact producer-local scene id".into());
                    }
                }
                Ok(anchors)
            },
        )
        .transpose()?;
    if exact_scene_anchors.is_none()
        && report
            .analysis
            .evidence
            .iter()
            .any(|item| item.modality == Modality::Scene)
    {
        return Err("direct-file scene producer completed without original PTS anchors".into());
    }
    let mut scenes = Vec::new();
    for item in &report.analysis.evidence {
        let EvidenceContent::Scene(scene) = &item.content else {
            continue;
        };
        let exact_anchor = exact_scene_anchors
            .as_ref()
            .map(|anchors| {
                anchors
                    .get(scene.scene_id.as_str())
                    .copied()
                    .ok_or_else(|| format!("no original PTS for scene {}", scene.scene_id))
            })
            .transpose()?;
        let (start, end) = scene_frame_positions(item, scene, exact_anchor)?;
        let related: BTreeSet<_> = report
            .analysis
            .relations
            .iter()
            .filter(|relation| relation.subject == item.id)
            .map(|relation| relation.object.as_str())
            .collect();
        let mut visible_text = Vec::new();
        let mut speech_items = Vec::new();
        for evidence in &report.analysis.evidence {
            if !related.contains(evidence.id.as_str()) {
                continue;
            }
            match &evidence.content {
                EvidenceContent::VisibleText(text) => visible_text.push(VideoTextEvidence {
                    id: evidence.id.clone(),
                    text: text.text.clone(),
                    start_seconds: evidence.start_seconds,
                    end_seconds: evidence.end_seconds,
                    track_id: text.track_id.clone(),
                    role: text.role.clone(),
                    observations: text
                        .observations
                        .iter()
                        .map(|observation| VideoTextObservation {
                            timestamp_seconds: observation.timestamp_seconds,
                            frame_index: observation.frame_index,
                            ocr_confidence: observation.ocr_confidence,
                        })
                        .collect(),
                    producer: producer(evidence),
                }),
                EvidenceContent::Speech(text) => speech_items.push(VideoSpeechEvidence {
                    id: evidence.id.clone(),
                    text: text.text.clone(),
                    start_seconds: evidence.start_seconds,
                    end_seconds: evidence.end_seconds,
                    transcript_confidence: text.transcript_confidence,
                    producer: producer(evidence),
                }),
                EvidenceContent::Scene(_) => {}
            }
        }
        let mut ids = related;
        ids.insert(&item.id);
        let relations = report
            .analysis
            .relations
            .iter()
            .filter(|relation| {
                ids.contains(relation.subject.as_str()) && ids.contains(relation.object.as_str())
            })
            .map(|relation| VideoEvidenceRelation {
                kind: match relation.kind {
                    RelationKind::SceneSpeech => "scene_speech",
                    RelationKind::SceneVisibleText => "scene_visible_text",
                    RelationKind::SpeechVisibleTextCooccurrence => {
                        "speech_visible_text_cooccurrence"
                    }
                }
                .into(),
                subject: relation.subject.clone(),
                object: relation.object.clone(),
            })
            .collect();
        // One compatibility text contribution per producer track, never per frame.
        let mut seen = BTreeSet::new();
        let frames: Vec<_> = visible_text
            .iter()
            .filter(|text| seen.insert(text.text.to_lowercase()))
            .map(|text| OcrFrameText {
                frame_index: text
                    .observations
                    .first()
                    .and_then(|observation| observation.frame_index)
                    .unwrap_or(0) as usize,
                text: text.text.clone(),
            })
            .collect();
        let ocr = OcrAnalysis {
            text: frames
                .iter()
                .map(|frame| frame.text.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            frames,
        };
        scenes.push(AnalyzedVideoScene {
            scene_index: scene.scene_index as usize,
            scene: Scene { start, end },
            ocr,
            evidence: VideoEvidence {
                asset_id: asset_id.clone(),
                content_hash: hash.clone(),
                scene_id: item.id.clone(),
                ocr_status: status(&report.visible_text),
                speech_status: status(&report.speech),
                visible_text,
                speech: speech_items,
                relations,
            },
        });
    }
    scenes.sort_by_key(|scene| scene.scene_index);
    if let Some(anchors) = &exact_scene_anchors {
        if anchors.len() != scenes.len() {
            return Err("exact scene anchors do not cover the complete producer run".into());
        }
    }
    if scenes.is_empty() {
        return Err("video analysis produced no source scenes".into());
    }
    Ok(scenes)
}

/// Convert between Foundation revisions by copying the *integer PTS and timebase*,
/// not through a floating-second or nanosecond approximation. This is an adapter
/// between crate instances, not a new time representation or clock rescaling.
fn original_timestamp(
    source: media_intelligence_analysis::ExactAnchorTimingV1,
    start: bool,
) -> Result<Timestamp, String> {
    let source = match source {
        ExactAnchorTimingV1::Range { start: s, end } => {
            if start {
                s
            } else {
                end
            }
        }
        _ => return Err("scene anchor is not a half-open exact range".into()),
    };
    source.validate().map_err(|error| error.to_string())?;
    // The older registry Foundation instance re-exported by this application's
    // Visual Analysis revision has only compatibility constructors. Validation
    // happens above in the canonical producer-owned Foundation contract.
    Ok(Timestamp::new(
        source.pts,
        Timebase::new(source.timebase.num, source.timebase.den),
    ))
}

fn scene_frame_positions(
    item: &EvidenceItem,
    scene: &SceneEvidence,
    anchor: Option<&ExactAnchorV1>,
) -> Result<(FramePosition, FramePosition), String> {
    let Some(anchor) = anchor else {
        // Explicit seconds-only compatibility fallback (#149), never advertised
        // as original PTS. Direct exact analysis fails closed if anchors vanish.
        return Ok((
            scene_frame_position(scene.start_frame, item.start_seconds)?,
            scene_frame_position(scene.end_frame, item.end_seconds)?,
        ));
    };
    if anchor.receipt != item.receipt
        || anchor.record_id != scene.scene_id
        || anchor.start_frame != scene.start_frame
        || anchor.end_frame != scene.end_frame
        || anchor.frame_index.is_some()
    {
        return Err(
            "original scene PTS has inconsistent producer receipt or frame provenance".into(),
        );
    }
    let ExactAnchorTimingV1::Range { start, end } = anchor.timing else {
        return Err("scene anchor is not an exact range".into());
    };
    if start.seconds() != item.start_seconds || end.seconds() != item.end_seconds {
        return Err("original scene PTS disagrees with v1 evidence projection".into());
    }
    if start
        .chronological_cmp(end)
        .map_err(|error| error.to_string())?
        != std::cmp::Ordering::Less
    {
        return Err("original scene PTS is not a nonempty half-open range".into());
    }
    Ok((
        FramePosition {
            frame_index: scene.start_frame.ok_or("missing original start frame")?,
            timestamp: original_timestamp(anchor.timing, true)?,
        },
        FramePosition {
            frame_index: scene
                .end_frame
                .ok_or("missing original exclusive end frame")?,
            timestamp: original_timestamp(anchor.timing, false)?,
        },
    ))
}

/// Project the seconds-only Media Intelligence v1 scene boundary into the
/// video-analysis FramePosition contract. This is nanosecond quantization,
/// not recovery of the producer's original rational PTS. Never silently
/// saturate the integer cast or invent a frame index on missing provenance.
fn scene_frame_position(frame: Option<u64>, seconds: f64) -> Result<FramePosition, String> {
    let frame_index = frame.ok_or("scene producer omitted frame provenance")?;
    if !seconds.is_finite() || seconds < 0.0 {
        return Err("scene producer returned an invalid source-relative time".into());
    }
    let nanoseconds = (seconds * 1_000_000_000.0).round();
    // Beyond 2^53 ticks, f64 no longer distinguishes adjacent nanoseconds.
    // An exact-PTS payload is required to support longer timelines losslessly.
    if !nanoseconds.is_finite() || nanoseconds > 9_007_199_254_740_991.0 {
        return Err("scene time exceeds the precise nanosecond projection range".into());
    }
    Ok(FramePosition {
        frame_index,
        timestamp: Timestamp::new(nanoseconds as i64, Timebase::new(1, 1_000_000_000)),
    })
}

fn content_hash(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path).map_err(|error| error.to_string())?;
    let mut hash = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        hash.update(&buffer[..count]);
    }
    Ok(hash
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn bridge_executable() -> Result<PathBuf, String> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    // Cargo integration tests run under target/<profile>/deps. Cargo builds the
    // service binary alongside them; production invokes the service itself.
    if executable
        .parent()
        .and_then(Path::file_name)
        .is_some_and(|name| name == "deps")
    {
        return Ok(executable
            .parent()
            .and_then(Path::parent)
            .ok_or("missing Cargo target directory")?
            .join("image-similarity-service"));
    }
    if executable
        .file_stem()
        .is_some_and(|name| name == "image-similarity-service")
    {
        return Ok(executable);
    }
    // Native quality/sample tools share these adapters and are distributed beside
    // the service binary. Their own CLI must not be mistaken for the OCR bridge.
    Ok(executable
        .parent()
        .ok_or("missing executable directory")?
        .join("image-similarity-service"))
}

fn producer(item: &EvidenceItem) -> EvidenceProducer {
    let receipt = &item.receipt;
    EvidenceProducer {
        producer: receipt.producer.clone(),
        producer_version: receipt.producer_version.clone(),
        model: receipt.model.clone(),
        model_version: receipt.model_version.clone(),
        config_hash: receipt.config_hash.clone(),
        input_hash: receipt.input_hash.clone(),
    }
}

fn status(status: &StageStatus) -> EvidenceStatus {
    match status {
        StageStatus::NotRequested => EvidenceStatus::NotRequested,
        StageStatus::Unavailable => EvidenceStatus::Unavailable,
        StageStatus::Failed { reason, .. } => EvidenceStatus::Failed {
            reason: reason.clone(),
        },
        StageStatus::Cancelled => EvidenceStatus::Failed {
            reason: "analysis cancelled".into(),
        },
        _ => EvidenceStatus::Completed,
    }
}

struct ExistingSpeech<'a> {
    audio: Option<&'a AudioAnalysis>,
    settings: &'a Settings,
}
impl SpeechProducer for ExistingSpeech<'_> {
    fn identity(&self) -> ProducerIdentity {
        ProducerIdentity {
            producer: "media-similarity:existing-transcript".into(),
            producer_version: env!("CARGO_PKG_VERSION").into(),
            model: self.settings.audio_transcription_model.clone(),
            model_version: "unreported-by-existing-transcript".into(),
            config_hash: format!(
                "sha256:{}",
                Sha256::digest(
                    format!(
                        "{}:{}",
                        self.settings.audio_transcription_model,
                        self.settings
                            .audio_transcription_language
                            .as_deref()
                            .unwrap_or_default()
                    )
                    .as_bytes()
                )
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
            ),
        }
    }
    fn run(&self, input: &ProducerInput<'_>) -> Result<SpeechRun, ProducerError> {
        let identity = self.identity();
        let audio = self.audio.ok_or_else(|| ProducerError::Failed {
            reason: "source transcript is unavailable".into(),
        })?;
        let segments = audio
            .transcript_segments
            .iter()
            .map(|segment| {
                let timing = || ProducerError::Failed {
                    reason: format!(
                        "transcript segment {} has no complete source-relative timing",
                        segment.segment_index
                    ),
                };
                Ok(TranscriptSegment {
                    index: segment.segment_index,
                    start_seconds: segment.start_seconds.ok_or_else(timing)?,
                    end_seconds: segment.end_seconds.ok_or_else(timing)?,
                    text: segment.text.clone(),
                    language: audio.transcript_language.clone(),
                    speaker: None,
                    confidence: segment.confidence.map(f64::from),
                })
            })
            .collect::<Result<Vec<_>, ProducerError>>()?;
        Ok(SpeechRun {
            receipt: ProducerReceipt {
                modality: Modality::Speech,
                producer: identity.producer,
                producer_version: identity.producer_version,
                model: identity.model,
                model_version: identity.model_version,
                config_hash: identity.config_hash,
                input_hash: input.input_hash.into(),
                run_id: None,
                config: None,
            },
            segments,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{analyze_video, scene_frame_position};
    use crate::config::Settings;
    use std::path::Path;
    use std::process::Command;

    /// Red 0.2 s followed by green 0.8 s at 100 Hz, dropping every sixth
    /// source frame. The 90 kHz PTS 18,000 is the cut, with 17 frames before
    /// it (above Media Intelligence's default minimum scene length).
    fn write_vfr_source(path: &Path, nonzero_origin: bool) -> bool {
        for name in ["ffmpeg", "ffprobe"] {
            if !Command::new(name)
                .arg("-version")
                .output()
                .is_ok_and(|result| result.status.success())
            {
                eprintln!("SKIPPED original source PTS replay: {name} unavailable");
                return false;
            }
        }
        let mut command = Command::new("ffmpeg");
        command.args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=red:s=64x64:r=100:d=0.2",
            "-f",
            "lavfi",
            "-i",
            "color=c=green:s=64x64:r=100:d=0.8",
            "-filter_complex",
            "[0:v][1:v]concat=n=2:v=1:a=0,select=not(eq(mod(n\\,6)\\,5))[v]",
            "-map",
            "[v]",
            "-fps_mode",
            "vfr",
            "-c:v",
            "libx264",
            "-threads",
            "1",
            "-pix_fmt",
            "yuv420p",
            "-video_track_timescale",
            "90000",
        ]);
        if nonzero_origin {
            command.args(["-output_ts_offset", "1"]);
        }
        assert!(command.arg(path).status().unwrap().success());
        true
    }

    #[test]
    fn real_vfr_scene_cut_keeps_original_integer_90000hz_pts_through_consumer() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source-vfr.mp4");
        if !write_vfr_source(&path, false) {
            return;
        }
        let settings = Settings {
            ocr_enabled: false,
            ..Settings::default()
        };
        let first = analyze_video(&path, "source-vfr", None, &settings).unwrap();
        assert_eq!(first.len(), 2, "real source scene detection");
        let scene_a = &first[0].scene;
        let scene_b = &first[1].scene;
        assert_eq!(scene_a.start.timestamp.pts, 0);
        assert_eq!(scene_a.end.timestamp.pts, 18_000);
        assert_eq!(scene_b.start.timestamp.pts, 18_000);
        assert_eq!(scene_b.end.timestamp.pts, 90_000);
        for boundary in [
            scene_a.start.timestamp,
            scene_a.end.timestamp,
            scene_b.start.timestamp,
            scene_b.end.timestamp,
        ] {
            assert_eq!((boundary.timebase.num, boundary.timebase.den), (1, 90_000));
        }
        assert_eq!(scene_a.end, scene_b.start, "cut is a shared exact boundary");
        assert!(scene_b.start.frame_index > 15);

        let replay = analyze_video(&path, "source-vfr", None, &settings).unwrap();
        assert_eq!(first.len(), replay.len());
        for (before, after) in first.iter().zip(&replay) {
            assert_eq!(before.scene, after.scene);
            assert_eq!(before.evidence, after.evidence);
        }
    }

    #[test]
    fn direct_vfr_consumer_rejects_unmapped_nonzero_source_clock() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("offset-source-vfr.mp4");
        if !write_vfr_source(&path, true) {
            return;
        }
        let settings = Settings {
            ocr_enabled: false,
            ..Settings::default()
        };
        let error = analyze_video(&path, "offset-vfr", None, &settings)
            .err()
            .expect("nonzero source PTS must fail closed");
        assert!(error.contains("nonzero"), "{error}");
    }

    #[test]
    fn vfr_scene_frame_positions_keep_source_frame_indices_and_cut_boundaries() {
        // Irregular 90 kHz PTS: there is intentionally no frame-rate shortcut.
        let frames = [
            (0_u64, 0_i64),
            (1, 3_003),
            (2, 7_100),
            (3, 12_000),
            (4, 18_000),
        ];
        for (index, pts) in frames {
            let seconds = pts as f64 / 90_000.0;
            let result = scene_frame_position(Some(index), seconds).unwrap();
            assert_eq!(result.frame_index, index);
            assert!((result.timestamp.seconds() - seconds).abs() <= 0.5e-9);
        }
        let before_cut = scene_frame_position(Some(4), 0.2).unwrap();
        let after_cut = scene_frame_position(Some(4), 0.2).unwrap();
        assert_eq!(before_cut, after_cut);
        assert_eq!(before_cut.timestamp.pts, 200_000_000);
    }

    #[test]
    fn seconds_projection_rejects_missing_or_unrepresentable_time() {
        assert!(scene_frame_position(None, 0.0).is_err());
        for seconds in [
            -1.0,
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            9_007_200.0,
        ] {
            assert!(scene_frame_position(Some(7), seconds).is_err());
        }
    }
}
