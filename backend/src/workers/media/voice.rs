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
    loaded_from_file: bool,
    path: std::path::PathBuf,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct VoiceRegistryMergeOutcome {
    pub changed: bool,
    pub warnings: Vec<String>,
}

impl VoiceRegistry {
    pub fn load(settings: &Settings) -> Result<Self, String> {
        let (library, loaded_from_file) = match fs::read_to_string(&settings.voice_registry_path) {
            Ok(json) => (
                SpeakerLibrary::from_json_str(&json).map_err(|error| error.to_string())?,
                true,
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                (SpeakerLibrary::new(), false)
            }
            Err(error) => return Err(error.to_string()),
        };
        let next_index = next_voice_index(&library);
        Ok(Self {
            library,
            embedder: SpectralSpeakerEmbedder::default(),
            next_index,
            changed: false,
            loaded_from_file,
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

    pub fn loaded_from_file(&self) -> bool {
        self.loaded_from_file
    }

    pub fn label(&self, id: &str) -> Result<Option<String>, String> {
        let id = SpeakerId::new(id.to_string()).map_err(|error| error.to_string())?;
        Ok(self
            .library
            .profile(&id)
            .map(|profile| profile.label().as_str().to_string()))
    }

    pub fn rename(&mut self, id: &str, label: &str) -> Result<bool, String> {
        let id = SpeakerId::new(id.to_string()).map_err(|error| error.to_string())?;
        let Some(existing) = self.library.profile(&id).cloned() else {
            return Ok(false);
        };
        if existing.label().as_str() == label {
            return Ok(false);
        }

        let mut next_library = SpeakerLibrary::new();
        for profile in self.library.profiles() {
            if profile.id() == &id {
                next_library
                    .add_profile(rebuild_profile(
                        profile,
                        profile.id().as_str(),
                        label,
                        true,
                    )?)
                    .map_err(|error| error.to_string())?;
            } else {
                next_library
                    .add_profile(profile.clone())
                    .map_err(|error| error.to_string())?;
            }
        }
        self.library = next_library;
        self.changed = true;
        Ok(true)
    }

    pub fn merge(
        &mut self,
        target_id: &str,
        source_ids: &[String],
        target_label: &str,
    ) -> Result<VoiceRegistryMergeOutcome, String> {
        let target_id_value =
            SpeakerId::new(target_id.to_string()).map_err(|error| error.to_string())?;
        let Some(target_profile) = self.library.profile(&target_id_value).cloned() else {
            return Ok(VoiceRegistryMergeOutcome {
                changed: false,
                warnings: vec![format!(
                    "voice registry profile `{target_id}` was not found"
                )],
            });
        };

        let mut warnings = Vec::new();
        let mut source_profiles = Vec::new();
        for source_id in source_ids {
            let source_id_value =
                SpeakerId::new(source_id.clone()).map_err(|error| error.to_string())?;
            match self.library.profile(&source_id_value).cloned() {
                Some(profile) => source_profiles.push(profile),
                None => warnings.push(format!(
                    "voice registry source profile `{source_id}` was not found"
                )),
            }
        }

        if source_profiles.is_empty() && target_profile.label().as_str() == target_label {
            return Ok(VoiceRegistryMergeOutcome {
                changed: false,
                warnings,
            });
        }

        let mut next_library = SpeakerLibrary::new();
        for profile in self.library.profiles() {
            if source_ids
                .iter()
                .any(|source_id| source_id == profile.id().as_str())
            {
                continue;
            }
            if profile.id() == &target_id_value {
                let mut merged = rebuild_profile(profile, target_id, target_label, true)?;
                for source_profile in &source_profiles {
                    for embedding in source_profile.embeddings() {
                        merged
                            .add_embedding(embedding.clone())
                            .map_err(|error| error.to_string())?;
                    }
                }
                next_library
                    .add_profile(merged)
                    .map_err(|error| error.to_string())?;
            } else {
                next_library
                    .add_profile(profile.clone())
                    .map_err(|error| error.to_string())?;
            }
        }
        self.library = next_library;
        self.changed = true;
        Ok(VoiceRegistryMergeOutcome {
            changed: true,
            warnings,
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

fn rebuild_profile(
    profile: &SpeakerProfile,
    id: &str,
    label: &str,
    copy_embeddings: bool,
) -> Result<SpeakerProfile, String> {
    let mut next = SpeakerProfile::new(
        SpeakerId::new(id.to_string()).map_err(|error| error.to_string())?,
        SpeakerLabel::new(label.to_string()).map_err(|error| error.to_string())?,
    );
    for (key, value) in profile.metadata_map() {
        next = next.metadata(key.clone(), value.clone());
    }
    if copy_embeddings {
        for embedding in profile.embeddings() {
            next.add_embedding(embedding.clone())
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(next)
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
    use audio_analysis_speakers::{
        SpeakerAudio, SpeakerEmbeddingExtractor, SpeakerId, SpeakerLabel, SpeakerLibrary,
        SpeakerProfile, SpectralSpeakerEmbedder,
    };

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

    #[test]
    fn voice_registry_rename_rebuilds_and_persists_profile() {
        let dir = std::env::temp_dir().join(format!("voice-registry-{}", uuid::Uuid::new_v4()));
        let settings = Settings {
            voice_registry_path: dir.join("voices.json"),
            ..Settings::default()
        };
        write_speaker_library(&settings, &[("voice-0001", "Voice 1", 180.0)]);

        let mut registry = VoiceRegistry::load(&settings).unwrap();
        assert!(registry.rename("voice-0001", "Alice").unwrap());
        registry.save_if_changed().unwrap();

        let reloaded = VoiceRegistry::load(&settings).unwrap();
        assert_eq!(
            reloaded.label("voice-0001").unwrap().as_deref(),
            Some("Alice")
        );
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn voice_registry_merge_combines_embeddings_and_removes_sources() {
        let dir = std::env::temp_dir().join(format!("voice-registry-{}", uuid::Uuid::new_v4()));
        let settings = Settings {
            voice_registry_path: dir.join("voices.json"),
            ..Settings::default()
        };
        write_speaker_library(
            &settings,
            &[
                ("voice-0001", "Voice 1", 180.0),
                ("voice-0002", "Voice 2", 420.0),
            ],
        );

        let mut registry = VoiceRegistry::load(&settings).unwrap();
        let outcome = registry
            .merge("voice-0001", &["voice-0002".to_string()], "Alice")
            .unwrap();
        assert!(outcome.changed);
        registry.save_if_changed().unwrap();

        let reloaded = VoiceRegistry::load(&settings).unwrap();
        let target_id = SpeakerId::new("voice-0001".to_string()).unwrap();
        let target = reloaded.library.profile(&target_id).unwrap();
        assert_eq!(target.label().as_str(), "Alice");
        assert_eq!(target.embeddings().len(), 2);
        assert!(reloaded
            .library
            .profile(&SpeakerId::new("voice-0002".to_string()).unwrap())
            .is_none());
        let _ = std::fs::remove_dir_all(dir);
    }

    fn write_speaker_library(settings: &Settings, profiles: &[(&str, &str, f32)]) {
        let mut library = SpeakerLibrary::new();
        let mut embedder = SpectralSpeakerEmbedder::default();
        for (id, label, frequency) in profiles {
            let embedding = embedder
                .embed_speaker(&speaker_audio(*frequency))
                .expect("speaker embedding should be generated");
            let profile = SpeakerProfile::new(
                SpeakerId::new((*id).to_string()).unwrap(),
                SpeakerLabel::new((*label).to_string()).unwrap(),
            )
            .with_embedding(embedding)
            .unwrap();
            library.add_profile(profile).unwrap();
        }
        if let Some(parent) = settings.voice_registry_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(
            &settings.voice_registry_path,
            library.to_json_string().unwrap(),
        )
        .unwrap();
    }

    fn speaker_audio(frequency: f32) -> SpeakerAudio<'static> {
        let sample_rate = 8_000;
        let samples = (0..sample_rate)
            .map(|index| {
                let phase =
                    index as f32 * 2.0 * std::f32::consts::PI * frequency / sample_rate as f32;
                phase.sin() * 0.2
            })
            .collect::<Vec<_>>();
        SpeakerAudio::mono(Box::leak(samples.into_boxed_slice()), sample_rate).unwrap()
    }
}
