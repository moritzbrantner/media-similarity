use image::RgbImage;
use image_analysis_core::{ImagePixelFormat, ImageView};
use image_analysis_embeddings::{ImageEmbedderBackend, OnnxImageEmbedder};
use model_runtime::{ModelBundle, ModelBundleFile, ModelBundleManifest, ModelTask};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use crate::config::Settings;
use crate::workers::media::embedder::ImageEmbedder;
use crate::workers::media::media::MediaFrame;
use crate::workers::media::models::{load_role_bundle, ModelRole};

const ONNX_EMBEDDING_TIMEOUT: Duration = Duration::from_secs(30);

pub trait VisualEmbeddingBackend: Send + Sync {
    fn model_name(&self) -> &str;
    fn vector_size(&self) -> usize;
    fn is_degraded(&self) -> bool {
        false
    }
    fn embed_image(&self, image: &RgbImage) -> Result<Vec<f32>, String>;

    fn embed_media(&self, frames: &[MediaFrame], motion_weight: f32) -> Result<Vec<f32>, String> {
        if frames.is_empty() {
            return Ok(vec![0.0; self.vector_size()]);
        }
        if frames.len() == 1 {
            return self.embed_image(&frames[0].image);
        }

        let mut vector = vec![0.0_f32; self.vector_size()];
        let total_weight = frames
            .iter()
            .map(|frame| frame.delay_ms.max(1) as f32)
            .sum::<f32>();
        for frame in frames {
            let frame_vector = self.embed_image(&frame.image)?;
            let weight = frame.delay_ms.max(1) as f32 / total_weight;
            for (index, value) in frame_vector.into_iter().enumerate().take(vector.len()) {
                vector[index] += value * weight;
            }
        }

        if motion_weight > 0.0 {
            let motion = LegacyColorEmbedder::new("motion", self.vector_size())
                .embed_media(frames, motion_weight)?;
            let motion_weight = motion_weight.clamp(0.0, 1.0);
            let content_weight = 1.0 - motion_weight;
            for index in 0..vector.len() {
                vector[index] = vector[index] * content_weight + motion[index] * motion_weight;
            }
        }

        normalize(&mut vector);
        Ok(vector)
    }
}

#[derive(Clone, Debug)]
pub struct LegacyColorEmbedder {
    model_name: String,
    inner: ImageEmbedder,
    vector_size: usize,
}

impl LegacyColorEmbedder {
    pub fn new(model_name: impl Into<String>, vector_size: usize) -> Self {
        let model_name = model_name.into();
        Self {
            inner: ImageEmbedder::new(model_name.clone(), vector_size),
            model_name,
            vector_size,
        }
    }
}

impl VisualEmbeddingBackend for LegacyColorEmbedder {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn vector_size(&self) -> usize {
        self.vector_size
    }

    fn is_degraded(&self) -> bool {
        true
    }

    fn embed_image(&self, image: &RgbImage) -> Result<Vec<f32>, String> {
        Ok(self.inner.encode(image))
    }

    fn embed_media(&self, frames: &[MediaFrame], motion_weight: f32) -> Result<Vec<f32>, String> {
        Ok(self.inner.encode_media(frames, motion_weight))
    }
}

#[derive(Debug)]
pub struct OnnxVisualEmbedder {
    model_name: String,
    model_path: std::path::PathBuf,
    preprocessor_path: std::path::PathBuf,
    vector_size: usize,
    settings: Settings,
    runner: std::sync::OnceLock<
        std::sync::Mutex<Result<OnnxImageEmbedder<runtime_onnx::OnnxSession>, String>>,
    >,
}

impl OnnxVisualEmbedder {
    pub fn new(settings: &Settings) -> Self {
        Self {
            model_name: settings.clip_model_name.clone(),
            model_path: settings.visual_embedding_model_path.clone(),
            preprocessor_path: settings.visual_embedding_preprocessor_path.clone(),
            vector_size: settings.visual_embedding_vector_size,
            settings: settings.clone(),
            runner: std::sync::OnceLock::new(),
        }
    }

    pub fn is_available(&self) -> bool {
        load_role_bundle(ModelRole::VisualEmbedding, &self.settings).is_ok()
            || (self.model_path.is_file() && self.preprocessor_path.is_file())
    }

    fn unavailable_error(&self) -> String {
        format!(
            "visual ONNX model is not available: expected model at {} and preprocessor at {}",
            self.model_path.display(),
            self.preprocessor_path.display()
        )
    }

    fn runner(
        &self,
    ) -> Result<
        std::sync::MutexGuard<'_, Result<OnnxImageEmbedder<runtime_onnx::OnnxSession>, String>>,
        String,
    > {
        let model_path = self.model_path.clone();
        let preprocessor_path = self.preprocessor_path.clone();
        let vector_size = self.vector_size;
        let settings = self.settings.clone();
        self.runner
            .get_or_init(|| {
                let runner = load_role_bundle(ModelRole::VisualEmbedding, &settings)
                    .and_then(|bundle| {
                        OnnxImageEmbedder::from_bundle(bundle).map_err(|error| error.to_string())
                    })
                    .or_else(|bundle_error| {
                        if model_path.is_file() && preprocessor_path.is_file() {
                            OnnxImageEmbedder::from_bundle(legacy_visual_embedding_bundle(
                                &model_path,
                                &preprocessor_path,
                                vector_size,
                            ))
                            .map_err(|error| error.to_string())
                        } else {
                            Err(bundle_error)
                        }
                    });
                std::sync::Mutex::new(runner)
            })
            .lock()
            .map_err(|_| "visual ONNX runner mutex was poisoned".to_string())
    }
}

impl VisualEmbeddingBackend for OnnxVisualEmbedder {
    fn model_name(&self) -> &str {
        &self.model_name
    }

    fn vector_size(&self) -> usize {
        self.vector_size
    }

    fn embed_image(&self, image: &RgbImage) -> Result<Vec<f32>, String> {
        if !self.is_available() {
            return Err(self.unavailable_error());
        }

        let mut runner = self.runner()?;
        let runner = runner.as_mut().map_err(|error| error.clone())?;
        let view = rgb_image_view(image)?;
        runner
            .embed_image(&view)
            .map(|embedding| embedding.vector)
            .map_err(|error| error.to_string())
    }
}

pub struct FallbackVisualEmbedder {
    primary: Arc<OnnxVisualEmbedder>,
    fallback: LegacyColorEmbedder,
    primary_disabled: AtomicBool,
    primary_timeout: Duration,
}

impl FallbackVisualEmbedder {
    pub fn new(settings: &Settings) -> Self {
        Self {
            primary: Arc::new(OnnxVisualEmbedder::new(settings)),
            fallback: LegacyColorEmbedder::new(
                format!("legacy-fallback:{}", settings.clip_model_name),
                settings.visual_embedding_vector_size,
            ),
            primary_disabled: AtomicBool::new(false),
            primary_timeout: ONNX_EMBEDDING_TIMEOUT,
        }
    }

    #[cfg(test)]
    fn with_timeout(settings: &Settings, timeout: Duration) -> Self {
        Self {
            primary: Arc::new(OnnxVisualEmbedder::new(settings)),
            fallback: LegacyColorEmbedder::new(
                format!("legacy-fallback:{}", settings.clip_model_name),
                settings.visual_embedding_vector_size,
            ),
            primary_disabled: AtomicBool::new(false),
            primary_timeout: timeout,
        }
    }

    fn primary_embed_image(&self, image: &RgbImage) -> Result<Vec<f32>, String> {
        if self.primary_disabled.load(Ordering::Relaxed) {
            return Err("visual ONNX embedding is disabled after an earlier failure".to_string());
        }
        let primary = self.primary.clone();
        let image = image.clone();
        run_with_timeout(self.primary_timeout, move || primary.embed_image(&image)).inspect_err(
            |error| {
                if error.contains("timed out") {
                    self.primary_disabled.store(true, Ordering::Relaxed);
                }
            },
        )?
    }

    fn primary_embed_media(
        &self,
        frames: &[MediaFrame],
        motion_weight: f32,
    ) -> Result<Vec<f32>, String> {
        if self.primary_disabled.load(Ordering::Relaxed) {
            return Err("visual ONNX embedding is disabled after an earlier failure".to_string());
        }
        let primary = self.primary.clone();
        let frames = frames.to_vec();
        run_with_timeout(self.primary_timeout, move || {
            primary.embed_media(&frames, motion_weight)
        })
        .inspect_err(|error| {
            if error.contains("timed out") {
                self.primary_disabled.store(true, Ordering::Relaxed);
            }
        })?
    }
}

impl VisualEmbeddingBackend for FallbackVisualEmbedder {
    fn model_name(&self) -> &str {
        if self.is_degraded() {
            self.fallback.model_name()
        } else {
            self.primary.model_name()
        }
    }

    fn vector_size(&self) -> usize {
        self.primary.vector_size()
    }

    fn is_degraded(&self) -> bool {
        self.primary_disabled.load(Ordering::Relaxed) || !self.primary.is_available()
    }

    fn embed_image(&self, image: &RgbImage) -> Result<Vec<f32>, String> {
        match self.primary_embed_image(image) {
            Ok(vector) => Ok(vector),
            Err(error) => {
                self.primary_disabled.store(true, Ordering::Relaxed);
                tracing::warn!(%error, "falling back to legacy visual embedding");
                self.fallback.embed_image(image)
            }
        }
    }

    fn embed_media(&self, frames: &[MediaFrame], motion_weight: f32) -> Result<Vec<f32>, String> {
        if frames.len() == 1 {
            return self.embed_image(&frames[0].image);
        }
        if self.primary_disabled.load(Ordering::Relaxed) {
            return self.fallback.embed_media(frames, motion_weight);
        }

        self.primary_embed_media(frames, motion_weight)
            .or_else(|error| {
                self.primary_disabled.store(true, Ordering::Relaxed);
                tracing::warn!(%error, "falling back to legacy visual media embedding");
                self.fallback.embed_media(frames, motion_weight)
            })
    }
}

fn run_with_timeout<T>(
    timeout: Duration,
    run: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String>
where
    T: Send + 'static,
{
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::spawn(move || {
        let _ = sender.send(run());
    });
    match receiver.recv_timeout(timeout) {
        Ok(output) => Ok(output),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(format!(
            "visual ONNX embedding timed out after {} seconds",
            timeout.as_secs()
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err("visual ONNX embedding ended without a result".to_string())
        }
    }
}

pub fn build_visual_embedder(settings: &Settings) -> std::sync::Arc<dyn VisualEmbeddingBackend> {
    if settings.visual_embedding_enabled
        && settings
            .visual_embedding_backend
            .eq_ignore_ascii_case("onnx")
    {
        std::sync::Arc::new(FallbackVisualEmbedder::new(settings))
    } else {
        std::sync::Arc::new(LegacyColorEmbedder::new(
            "legacy-disabled",
            settings.visual_embedding_vector_size,
        ))
    }
}

fn normalize(vector: &mut [f32]) {
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in vector {
            *value /= norm;
        }
    }
}

fn rgb_image_view(image: &RgbImage) -> Result<ImageView<'_>, String> {
    ImageView::packed(
        image.width(),
        image.height(),
        ImagePixelFormat::Rgb24,
        image.as_raw(),
    )
    .map_err(|error| error.to_string())
}

fn legacy_visual_embedding_bundle(
    model_path: &Path,
    preprocessor_path: &Path,
    vector_size: usize,
) -> ModelBundle {
    let mut files = BTreeMap::new();
    files.insert(
        "model.onnx".to_string(),
        bundle_file("model.onnx", model_path),
    );
    files.insert(
        "preprocessor_config.json".to_string(),
        bundle_file("preprocessor_config.json", preprocessor_path),
    );
    files.insert(
        "config.json".to_string(),
        ModelBundleFile {
            remote_path: "config.json".to_string(),
            local_path: legacy_config_path(model_path, vector_size)
                .to_string_lossy()
                .into_owned(),
            size_bytes: 0,
        },
    );
    ModelBundle {
        root: PathBuf::new(),
        manifest: ModelBundleManifest {
            schema_version: 1,
            name: "legacy-visual-embedding".to_string(),
            repo_id: "legacy/visual-embedding".to_string(),
            revision: "local".to_string(),
            task: ModelTask::ImageEmbedding,
            files,
        },
    }
}

fn legacy_config_path(model_path: &Path, vector_size: usize) -> PathBuf {
    let config_path = std::env::temp_dir().join(format!(
        "media-similarity-legacy-visual-embedding-{vector_size}.json"
    ));
    if !config_path.is_file() {
        let _ = std::fs::write(
            &config_path,
            format!(r#"{{"projection_dim":{vector_size}}}"#),
        );
    }
    if config_path.is_file() {
        config_path
    } else {
        model_path.with_file_name("config.json")
    }
}

fn bundle_file(remote_path: &str, local_path: &Path) -> ModelBundleFile {
    ModelBundleFile {
        remote_path: remote_path.to_string(),
        local_path: local_path.to_string_lossy().into_owned(),
        size_bytes: std::fs::metadata(local_path)
            .map(|metadata| metadata.len())
            .unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgb};

    use std::sync::atomic::Ordering;
    use std::time::Duration;

    use super::{
        build_visual_embedder, run_with_timeout, FallbackVisualEmbedder, LegacyColorEmbedder,
        OnnxVisualEmbedder, VisualEmbeddingBackend,
    };
    use crate::config::Settings;

    #[test]
    fn legacy_color_embedder_returns_normalized_vectors() {
        let image = ImageBuffer::from_pixel(8, 8, Rgb([200, 20, 40]));
        let embedder = LegacyColorEmbedder::new("test", 32);

        let vector = embedder.embed_image(&image).unwrap();

        assert_eq!(vector.len(), 32);
        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.0001);
    }

    #[test]
    fn onnx_visual_embedder_reports_missing_model_files() {
        let settings = Settings {
            visual_embedding_model_path: std::env::temp_dir().join("missing-clip-model.onnx"),
            visual_embedding_preprocessor_path: std::env::temp_dir()
                .join("missing-preprocessor-config.json"),
            visual_embedding_vector_size: 16,
            ..Settings::default()
        };
        let image = ImageBuffer::from_pixel(4, 4, Rgb([10, 20, 30]));
        let embedder = OnnxVisualEmbedder::new(&settings);

        let error = embedder.embed_image(&image).unwrap_err();

        assert!(error.contains("visual ONNX model is not available"));
    }

    #[test]
    fn build_visual_embedder_falls_back_when_onnx_model_is_unavailable() {
        let settings = Settings {
            visual_embedding_model_path: std::env::temp_dir().join("missing-clip-model.onnx"),
            visual_embedding_preprocessor_path: std::env::temp_dir()
                .join("missing-preprocessor-config.json"),
            visual_embedding_vector_size: 16,
            ..Settings::default()
        };
        let image = ImageBuffer::from_pixel(4, 4, Rgb([10, 20, 30]));
        let embedder = build_visual_embedder(&settings);

        let vector = embedder.embed_image(&image).unwrap();

        assert_eq!(vector.len(), 16);
    }

    #[test]
    fn fallback_visual_embedder_uses_legacy_after_primary_timeout() {
        let settings = Settings {
            visual_embedding_model_path: std::env::temp_dir().join("missing-clip-model.onnx"),
            visual_embedding_preprocessor_path: std::env::temp_dir()
                .join("missing-preprocessor-config.json"),
            visual_embedding_vector_size: 16,
            ..Settings::default()
        };
        let embedder = FallbackVisualEmbedder::with_timeout(&settings, Duration::from_millis(1));
        embedder.primary_disabled.store(true, Ordering::Relaxed);
        let image = ImageBuffer::from_pixel(4, 4, Rgb([10, 20, 30]));

        let vector = embedder.embed_image(&image).unwrap();

        assert_eq!(vector.len(), 16);
        assert!(embedder.model_name().starts_with("legacy-fallback:"));
    }

    #[test]
    fn run_with_timeout_reports_slow_work() {
        let error = run_with_timeout(Duration::from_millis(1), || {
            std::thread::sleep(Duration::from_millis(20));
        })
        .unwrap_err();

        assert!(error.contains("timed out"));
    }
}
