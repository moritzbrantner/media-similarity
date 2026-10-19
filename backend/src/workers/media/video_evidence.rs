//! Translate the public composition facade into application-owned video metadata.
use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use media_intelligence_analysis::visible_text::{
    OcrBackendConfig, VisualAnalysisOcrConfig, VisualAnalysisOcrProducer,
};
use media_intelligence_analysis::{
    analyze, AnalysisProfile, AnalysisRequest, AssetId, CancellationToken, ContentHash,
    InMemoryStageCache, MediaIdentity, Modality, ProducerError, ProducerIdentity, ProducerInput,
    ProducerReceipt, Producers, SceneConfig, SpeechProducer, SpeechRun, StageStatus,
};
use media_intelligence_core::{EvidenceContent, EvidenceItem, RelationKind, TranscriptSegment};
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
    let report = analyze(
        &request,
        &Producers {
            speech: audio.map(|_| &speech as &dyn SpeechProducer),
            visible_text: settings.ocr_enabled.then_some(&ocr),
        },
        &mut InMemoryStageCache::new(),
        &CancellationToken::new(),
    )
    .map_err(|error| error.to_string())?;
    if let StageStatus::Failed { reason, .. } = &report.scenes {
        return Err(reason.clone());
    }
    let mut scenes = Vec::new();
    for item in &report.analysis.evidence {
        let EvidenceContent::Scene(scene) = &item.content else {
            continue;
        };
        let frame_position = |frame: Option<u64>, seconds: f64| -> Result<FramePosition, String> {
            Ok(FramePosition {
                frame_index: frame.ok_or("scene producer omitted frame provenance")?,
                timestamp: Timestamp::new(
                    (seconds * 1_000_000_000.0).round() as i64,
                    Timebase::new(1, 1_000_000_000),
                ),
            })
        };
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
            scene: Scene {
                start: frame_position(scene.start_frame, item.start_seconds)?,
                end: frame_position(scene.end_frame, item.end_seconds)?,
            },
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
    if scenes.is_empty() {
        return Err("video analysis produced no source scenes".into());
    }
    Ok(scenes)
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
