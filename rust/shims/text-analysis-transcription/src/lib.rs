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

    pub fn file_name(self) -> String {
        format!("ggml-{}.bin", self.id())
    }

    pub fn download_url(self) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            self.file_name()
        )
    }

    pub fn checksum_sha256(self) -> &'static str {
        match self {
            Self::TinyEn => "0d686a2a6a22b02da2ef3101d4c86e68461363a623c58f27f81b1b2d36b42317",
            Self::Tiny => "518970a29bedb265f23ac48d486ddbc63bedffd90967b10140ae5ac61243acf3",
            Self::BaseEn => "a03779c86df3323075f5e796cb2ce5029f00ec8869eee3fdfb897afe36c6d002",
            Self::Base => "2f62d18b50c3f3feafbf990eec23a93d319660b1efbdd3fff55e52b7cde2e374",
            Self::SmallEn => "0d57184d34ae7d736e5bb2db5bf83debe730bd53dcefa235a0979b9dcfd33fb3",
            Self::Small => "edd29d67e70b000132af65205b99bb774b77abc13d10103e14f80ce2242913e1",
            Self::MediumEn => "a163589aa264d5188df3b05ed4eac56bfd97e26910f207809d869f7e99886fd2",
            Self::Medium => "d3d5696e6a3e0ca2aa08eb31cad208ffa1e87b3cc341f59e628fbdcf8122de9b",
            Self::LargeV1 => "cbcb187d1e1abe979d33636cdc63381de20738eeda0885c39440b086e184248a",
            Self::LargeV2 => "c6d6d3dcebc5e0074175386e17eba305fc5cc7d3d5dff3ecfd11e8f2bd4222d7",
            Self::LargeV3 => "766d11cebbdf5a67c179c5774e2642b609e35e1a30240e7b559d5647c655b0a4",
            Self::LargeV3Turbo => {
                "5a4b65b05933d70ce9d5aa6265eb128fa5eba38f6fee40836fdedc4d2fde42ad"
            }
        }
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

    pub fn models_dir(&self) -> PathBuf {
        self.root.join("models")
    }

    pub fn model_path(&self, model: WhisperCppModel) -> PathBuf {
        self.models_dir().join(model.file_name())
    }

    pub fn catalog(&self) -> WhisperCppCatalog {
        WhisperCppCatalog {
            default_model: WhisperCppModel::BaseEn,
            models: WhisperCppModel::ALL
                .into_iter()
                .map(|model| WhisperCppModelStatus {
                    model,
                    cached: self.model_path(model).is_file(),
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
