use std::fmt;
use std::path::{Path, PathBuf};

pub type Result<T> = std::result::Result<T, TranscriptionError>;

#[derive(Debug, Clone)]
pub struct TranscriptionError {
    message: String,
}

impl fmt::Display for TranscriptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for TranscriptionError {}

#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptionResult {
    pub text: Option<String>,
    pub language: Option<String>,
    pub segments: Vec<TranscriptSegment>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TranscriptSegment {
    pub index: u64,
    pub start_seconds: Option<f64>,
    pub end_seconds: Option<f64>,
    pub text: String,
    pub confidence: Option<f32>,
}

pub trait Transcriber {
    fn transcribe(&mut self, input: &Path) -> Result<TranscriptionResult>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhisperCppModel {
    TinyEn,
    Tiny,
    BaseEn,
    Base,
    SmallEn,
    Small,
    MediumEn,
    Medium,
    LargeV1,
    LargeV2,
    LargeV3,
    LargeV3Turbo,
}

impl WhisperCppModel {
    pub const ALL: [Self; 12] = [
        Self::TinyEn,
        Self::Tiny,
        Self::BaseEn,
        Self::Base,
        Self::SmallEn,
        Self::Small,
        Self::MediumEn,
        Self::Medium,
        Self::LargeV1,
        Self::LargeV2,
        Self::LargeV3,
        Self::LargeV3Turbo,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Self::TinyEn => "tiny.en",
            Self::Tiny => "tiny",
            Self::BaseEn => "base.en",
            Self::Base => "base",
            Self::SmallEn => "small.en",
            Self::Small => "small",
            Self::MediumEn => "medium.en",
            Self::Medium => "medium",
            Self::LargeV1 => "large-v1",
            Self::LargeV2 => "large-v2",
            Self::LargeV3 => "large-v3",
            Self::LargeV3Turbo => "large-v3-turbo",
        }
    }

    fn file_name(self) -> String {
        format!("ggml-{}.bin", self.id())
    }
}

#[derive(Debug, Clone)]
pub struct WhisperCppConfig {
    pub model: WhisperCppModel,
    pub language: Option<String>,
    pub translate: bool,
    pub threads: Option<usize>,
}

#[derive(Debug, Clone)]
pub struct WhisperCppModelStatus {
    pub model: WhisperCppModel,
    pub cached: bool,
}

#[derive(Debug, Clone)]
pub struct WhisperCppCatalog {
    pub default_model: WhisperCppModel,
    pub models: Vec<WhisperCppModelStatus>,
}

#[derive(Debug, Clone)]
pub struct WhisperCppModelStore {
    root: PathBuf,
}

impl Default for WhisperCppModelStore {
    fn default() -> Self {
        Self::new(default_cache_dir())
    }
}

impl WhisperCppModelStore {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn catalog(&self) -> WhisperCppCatalog {
        WhisperCppCatalog {
            default_model: WhisperCppModel::BaseEn,
            models: WhisperCppModel::ALL
                .into_iter()
                .map(|model| WhisperCppModelStatus {
                    model,
                    cached: self.root.join("models").join(model.file_name()).is_file(),
                })
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct WhisperCppTranscriber {
    config: WhisperCppConfig,
    store: WhisperCppModelStore,
}

impl WhisperCppTranscriber {
    pub fn new(config: WhisperCppConfig) -> Self {
        Self {
            config,
            store: WhisperCppModelStore::default(),
        }
    }

    pub fn with_model_store(mut self, store: WhisperCppModelStore) -> Self {
        self.store = store;
        self
    }
}

impl Transcriber for WhisperCppTranscriber {
    fn transcribe(&mut self, _input: &Path) -> Result<TranscriptionResult> {
        let _ = &self.store;
        Err(TranscriptionError {
            message: format!(
                "native whisper.cpp transcription is unavailable in the repository-local text compatibility layer for model `{}`",
                self.config.model.id()
            ),
        })
    }
}

fn default_cache_dir() -> PathBuf {
    std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache")))
        .unwrap_or_else(std::env::temp_dir)
        .join("video-analysis-studio")
        .join("whisper-cpp")
}

#[cfg(test)]
mod tests {
    use super::{WhisperCppModel, WhisperCppModelStore};

    #[test]
    fn parses_expected_model_ids() {
        assert!(WhisperCppModel::ALL
            .into_iter()
            .any(|model| model.id() == "base.en"));
    }

    #[test]
    fn catalog_reports_all_models() {
        let store = WhisperCppModelStore::new(std::env::temp_dir().join("missing-model-cache"));
        assert_eq!(store.catalog().models.len(), WhisperCppModel::ALL.len());
    }
}
