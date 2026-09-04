use image::{imageops::FilterType, DynamicImage};
use wasm_bindgen::prelude::*;

const SIGNATURE_GRID: u32 = 12;
const CHANNELS: usize = 3;

#[wasm_bindgen]
pub struct SimilarityIndex {
    signature_len: Option<usize>,
    signatures: Vec<Vec<f32>>,
}

#[wasm_bindgen]
impl SimilarityIndex {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            signature_len: None,
            signatures: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.signatures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.signatures.is_empty()
    }

    pub fn add(&mut self, image_bytes: &[u8]) -> Result<u32, JsValue> {
        let signature = signature_from_bytes(image_bytes).map_err(decode_error)?;
        self.push_signature(signature).map_err(signature_error)
    }

    /// Add a normalized browser-extracted feature vector to this index.
    ///
    /// Every vector in one index must use the same dimension and contain finite values in
    /// 0.0..=1.0.
    #[wasm_bindgen(js_name = addFeatures)]
    pub fn add_features(&mut self, features: &[f32]) -> Result<u32, JsValue> {
        self.push_signature(features.to_vec()).map_err(signature_error)
    }

    /// Returns flattened `(index, score)` pairs sorted by descending similarity.
    /// Scores are normalized to the inclusive range 0.0..=1.0.
    pub fn search(&self, query_image_bytes: &[u8], limit: usize) -> Result<Vec<f64>, JsValue> {
        let query = signature_from_bytes(query_image_bytes).map_err(decode_error)?;
        self.rank_query(&query, limit).map_err(signature_error)
    }

    /// Rank normalized browser-extracted feature vectors with the same deterministic core used by
    /// images.
    #[wasm_bindgen(js_name = searchFeatures)]
    pub fn search_features(
        &self,
        query_features: &[f32],
        limit: usize,
    ) -> Result<Vec<f64>, JsValue> {
        self.rank_query(query_features, limit).map_err(signature_error)
    }
}

impl SimilarityIndex {
    fn push_signature(&mut self, signature: Vec<f32>) -> Result<u32, String> {
        validate_signature(&signature, self.signature_len)?;
        let index = self.signatures.len();
        self.signature_len = Some(signature.len());
        self.signatures.push(signature);
        u32::try_from(index).map_err(|_| "similarity index is too large".to_owned())
    }

    fn rank_query(&self, query: &[f32], limit: usize) -> Result<Vec<f64>, String> {
        validate_signature(query, self.signature_len)?;
        Ok(rank_signatures(query, &self.signatures, limit))
    }
}

impl Default for SimilarityIndex {
    fn default() -> Self {
        Self::new()
    }
}

fn decode_error(error: image::ImageError) -> JsValue {
    JsValue::from_str(&format!("could not decode image: {error}"))
}

fn signature_error(error: String) -> JsValue {
    JsValue::from_str(&error)
}

fn signature_from_bytes(bytes: &[u8]) -> Result<Vec<f32>, image::ImageError> {
    image::load_from_memory(bytes).map(|image| signature_from_image(&image))
}

fn signature_from_image(image: &DynamicImage) -> Vec<f32> {
    let resized = image
        .resize_exact(SIGNATURE_GRID, SIGNATURE_GRID, FilterType::Triangle)
        .to_rgb8();
    let mut signature = Vec::with_capacity((SIGNATURE_GRID * SIGNATURE_GRID) as usize * CHANNELS);

    for pixel in resized.pixels() {
        signature.extend(pixel.0.map(|channel| f32::from(channel) / 255.0));
    }

    signature
}

fn validate_signature(signature: &[f32], expected_len: Option<usize>) -> Result<(), String> {
    if signature.is_empty() {
        return Err("feature vector must not be empty".to_owned());
    }

    match expected_len {
        Some(expected_len) if signature.len() != expected_len => {
            return Err(format!(
                "feature vector dimension mismatch: expected {expected_len}, got {}",
                signature.len()
            ));
        }
        _ => {}
    }

    if signature.iter().any(|value| !value.is_finite()) {
        return Err("feature vector values must be finite".to_owned());
    }

    if signature.iter().any(|value| !(0.0..=1.0).contains(value)) {
        return Err("feature vector values must be normalized to 0.0..=1.0".to_owned());
    }

    Ok(())
}

fn similarity(left: &[f32], right: &[f32]) -> f32 {
    debug_assert_eq!(left.len(), right.len());
    if left.is_empty() || left.len() != right.len() {
        return 0.0;
    }

    let mean_absolute_difference = left
        .iter()
        .zip(right)
        .map(|(left, right)| (left - right).abs())
        .sum::<f32>()
        / left.len() as f32;

    (1.0 - mean_absolute_difference).clamp(0.0, 1.0)
}

fn rank_signatures(query: &[f32], candidates: &[Vec<f32>], limit: usize) -> Vec<f64> {
    let mut ranked = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| (index, similarity(query, candidate)))
        .collect::<Vec<_>>();

    ranked.sort_by(|left, right| {
        right
            .1
            .total_cmp(&left.1)
            .then_with(|| left.0.cmp(&right.0))
    });

    ranked
        .into_iter()
        .take(limit.min(candidates.len()))
        .flat_map(|(index, score)| [index as f64, f64::from(score)])
        .collect()
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use image::{DynamicImage, ImageBuffer, ImageFormat, Rgb};

    use super::{
        rank_signatures, signature_from_bytes, signature_from_image, similarity, validate_signature,
    };

    fn png_bytes(color: [u8; 3]) -> Vec<u8> {
        let image = ImageBuffer::from_pixel(48, 32, Rgb(color));
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .unwrap();
        cursor.into_inner()
    }

    #[test]
    fn exact_images_score_one() {
        let image = DynamicImage::ImageRgb8(ImageBuffer::from_pixel(24, 24, Rgb([20, 80, 140])));
        let signature = signature_from_image(&image);
        assert_eq!(similarity(&signature, &signature), 1.0);
    }

    #[test]
    fn ranking_prefers_the_closest_color_family() {
        let query = signature_from_bytes(&png_bytes([220, 110, 70])).unwrap();
        let close = signature_from_bytes(&png_bytes([210, 105, 75])).unwrap();
        let far = signature_from_bytes(&png_bytes([30, 80, 210])).unwrap();
        let ranked = rank_signatures(&query, &[far, close], 2);

        assert_eq!(ranked[0], 1.0);
        assert!(ranked[1] > ranked[3]);
    }

    #[test]
    fn browser_features_use_the_same_ranking_core() {
        let query = vec![0.12, 0.48, 0.81, 0.25];
        let close = vec![0.10, 0.50, 0.79, 0.28];
        let far = vec![0.85, 0.05, 0.10, 0.90];
        let ranked = rank_signatures(&query, &[far, close], 2);

        assert_eq!(ranked[0], 1.0);
        assert!(ranked[1] > ranked[3]);
    }

    #[test]
    fn browser_features_require_stable_normalized_dimensions() {
        assert!(validate_signature(&[], None).is_err());
        assert!(validate_signature(&[0.1, f32::NAN], None).is_err());
        assert!(validate_signature(&[0.1, 1.1], None).is_err());
        assert!(validate_signature(&[0.1, 0.2], Some(3)).is_err());
        assert!(validate_signature(&[0.1, 0.2], Some(2)).is_ok());
    }

    #[test]
    fn malformed_input_is_rejected() {
        assert!(signature_from_bytes(b"not an image").is_err());
    }

    #[test]
    fn ranking_limit_is_bounded_by_candidate_count() {
        let query = signature_from_bytes(&png_bytes([120, 120, 120])).unwrap();
        let candidate = signature_from_bytes(&png_bytes([125, 125, 125])).unwrap();
        let ranked = rank_signatures(&query, &[candidate], 10);
        assert_eq!(ranked.len(), 2);
    }
}
