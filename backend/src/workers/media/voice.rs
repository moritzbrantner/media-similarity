use std::fs;

use audio_analysis_speakers::{
    SpeakerAudio, SpeakerConfidence, SpeakerEmbeddingExtractor, SpeakerId,
    SpeakerIdentificationOptions, SpeakerLabel, SpeakerLibrary, SpeakerProfile,
    SpectralSpeakerEmbedder, UnknownSpeakerPolicy,
};

use crate::config::Settings;

#[derive(Clone, Debug, PartialEq)]
pub struct VoiceRegistryMatch {
    pub id: String,
    pub label: String,
    pub score: f32,
    pub confidence: String,
}

pub struct VoiceRegistry {
    library: SpeakerLibrary,
    embedder: SpectralSpeakerEmbedder,
    next_index: u32,
    changed: bool,
    path: std::path::PathBuf,
}

impl VoiceRegistry {
    pub fn load(settings: &Settings) -> Result<Self, String> {
        let library = match fs::read_to_string(&settings.voice_registry_path) {
            Ok(json) => SpeakerLibrary::from_json_str(&json).map_err(|error| error.to_string())?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => SpeakerLibrary::new(),
            Err(error) => return Err(error.to_string()),
        };
        let next_index = next_voice_index(&library);
        Ok(Self {
            library,
            embedder: SpectralSpeakerEmbedder::default(),
            next_index,
            changed: false,
            path: settings.voice_registry_path.clone(),
        })
    }

    pub fn recognize_or_enroll(
        &mut self,
        samples: &[f32],
        sample_rate: u32,
    ) -> Result<VoiceRegistryMatch, String> {
        let audio = SpeakerAudio::mono(samples, sample_rate)
            .and_then(|audio| audio.duration_bounds(Some(0.25), Some(30.0)))
            .map_err(|error| error.to_string())?;
        let embedding = self
            .embedder
            .embed_speaker(&audio)
            .map_err(|error| error.to_string())?;
        let options = SpeakerIdentificationOptions {
            min_score: 0.86,
            min_margin: None,
            max_results: 5,
            unknown_policy: UnknownSpeakerPolicy::NoMatch,
        };
        let identified = self
            .library
            .identify(&embedding, &options)
            .map_err(|error| error.to_string())?;

        if let Some(best) = identified.best_match {
            return Ok(VoiceRegistryMatch {
                id: best.speaker_id.as_str().to_string(),
                label: best.label.as_str().to_string(),
                score: best.score,
                confidence: confidence_label(best.confidence).to_string(),
            });
        }

        let voice_index = self.next_index;
        self.next_index += 1;
        let id = format!("voice-{voice_index:04}");
        let label = format!("Voice {voice_index}");
        let profile = SpeakerProfile::new(
            SpeakerId::new(id.clone()).map_err(|error| error.to_string())?,
            SpeakerLabel::new(label.clone()).map_err(|error| error.to_string())?,
        )
        .with_embedding(embedding)
        .map_err(|error| error.to_string())?;
        self.library
            .add_profile(profile)
            .map_err(|error| error.to_string())?;
        self.changed = true;

        Ok(VoiceRegistryMatch {
            id,
            label,
            score: 1.0,
            confidence: "new".to_string(),
        })
    }

    pub fn save_if_changed(&self) -> Result<(), String> {
        if !self.changed {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let json = self
            .library
            .to_json_string()
            .map_err(|error| error.to_string())?;
        fs::write(&self.path, json).map_err(|error| error.to_string())
    }
}

fn next_voice_index(library: &SpeakerLibrary) -> u32 {
    library
        .profiles()
        .filter_map(|profile| profile.id().as_str().strip_prefix("voice-"))
        .filter_map(|suffix| suffix.parse::<u32>().ok())
        .max()
        .unwrap_or(0)
        + 1
}

fn confidence_label(confidence: SpeakerConfidence) -> &'static str {
    match confidence {
        SpeakerConfidence::High => "high",
        SpeakerConfidence::Medium => "medium",
        SpeakerConfidence::Low => "low",
    }
}

#[cfg(test)]
mod tests {
    use super::VoiceRegistry;
    use crate::config::Settings;

    #[test]
    fn voice_registry_enrolls_and_recognizes_repeat_voice() {
        let dir = std::env::temp_dir().join(format!("voice-registry-{}", uuid::Uuid::new_v4()));
        let settings = Settings {
            voice_registry_path: dir.join("voices.json"),
            ..Settings::default()
        };
        let sample_rate = 8_000;
        let samples = (0..sample_rate)
            .map(|index| {
                let phase = index as f32 * 2.0 * std::f32::consts::PI * 180.0 / sample_rate as f32;
                phase.sin() * 0.2
            })
            .collect::<Vec<_>>();

        let mut registry = VoiceRegistry::load(&settings).unwrap();
        let first = registry.recognize_or_enroll(&samples, sample_rate).unwrap();
        let second = registry.recognize_or_enroll(&samples, sample_rate).unwrap();
        registry.save_if_changed().unwrap();

        assert_eq!(first.id, "voice-0001");
        assert_eq!(second.id, "voice-0001");
        assert!(settings.voice_registry_path.exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
