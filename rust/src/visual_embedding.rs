use image::RgbImage;
use image_analysis_core::{ImagePixelFormat, ImageView};
use image_analysis_models::ImageEmbedderBackend;
use image_analysis_onnx::{NativeOnnxRunner, OnnxImageEmbedder};

use crate::config::Settings;
use crate::embedder::ImageEmbedder;
use crate::media::MediaFrame;

pub trait VisualEmbeddingBackend: Send + Sync {
    fn model_name(&self) -> &str;
    fn vector_size(&self) -> usize;
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
    runner:
        std::sync::OnceLock<std::sync::Mutex<Result<OnnxImageEmbedder<NativeOnnxRunner>, String>>>,
}

impl OnnxVisualEmbedder {
    pub fn new(settings: &Settings) -> Self {
        Self {
            model_name: settings.clip_model_name.clone(),
            model_path: settings.visual_embedding_model_path.clone(),
            preprocessor_path: settings.visual_embedding_preprocessor_path.clone(),
            vector_size: settings.visual_embedding_vector_size,
            runner: std::sync::OnceLock::new(),
        }
    }

    pub fn is_available(&self) -> bool {
        self.model_path.is_file() && self.preprocessor_path.is_file()
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
        std::sync::MutexGuard<'_, Result<OnnxImageEmbedder<NativeOnnxRunner>, String>>,
        String,
    > {
        let model_path = self.model_path.clone();
        let preprocessor_path = self.preprocessor_path.clone();
        let vector_size = self.vector_size;
        self.runner
            .get_or_init(|| {
                std::sync::Mutex::new(
                    OnnxImageEmbedder::from_paths(model_path, preprocessor_path, Some(vector_size))
                        .map_err(|error| error.to_string()),
                )
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
    primary: OnnxVisualEmbedder,
    fallback: LegacyColorEmbedder,
}

impl FallbackVisualEmbedder {
    pub fn new(settings: &Settings) -> Self {
        Self {
            primary: OnnxVisualEmbedder::new(settings),
            fallback: LegacyColorEmbedder::new(
                format!("legacy-fallback:{}", settings.clip_model_name),
                settings.visual_embedding_vector_size,
            ),
        }
    }
}

impl VisualEmbeddingBackend for FallbackVisualEmbedder {
    fn model_name(&self) -> &str {
        self.primary.model_name()
    }

    fn vector_size(&self) -> usize {
        self.primary.vector_size()
    }

    fn embed_image(&self, image: &RgbImage) -> Result<Vec<f32>, String> {
        match self.primary.embed_image(image) {
            Ok(vector) => Ok(vector),
            Err(error) => {
                tracing::warn!(%error, "falling back to legacy visual embedding");
                self.fallback.embed_image(image)
            }
        }
    }

    fn embed_media(&self, frames: &[MediaFrame], motion_weight: f32) -> Result<Vec<f32>, String> {
        match self.primary.embed_media(frames, motion_weight) {
            Ok(vector) => Ok(vector),
            Err(error) => {
                tracing::warn!(%error, "falling back to legacy visual media embedding");
                self.fallback.embed_media(frames, motion_weight)
            }
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

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgb};

    use super::{build_visual_embedder, LegacyColorEmbedder, VisualEmbeddingBackend};
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
    fn onnx_visual_embedder_falls_back_when_model_files_are_missing() {
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
        assert!(vector.iter().all(|value| value.is_finite()));
    }
}
