use image::RgbImage;

#[derive(Clone, Debug)]
pub struct ImageEmbedder {
    vector_size: usize,
}

impl ImageEmbedder {
    pub fn new(_model_name: impl Into<String>, vector_size: usize) -> Self {
        Self { vector_size }
    }

    pub fn encode(&self, image: &RgbImage) -> Vec<f32> {
        let mut vector = vec![0.0_f32; self.vector_size];
        if self.vector_size == 0 {
            return vector;
        }

        for (index, pixel) in image.pixels().enumerate() {
            let bucket = index % self.vector_size;
            vector[bucket] += f32::from(pixel[0]) / 255.0;
            vector[(bucket + 1) % self.vector_size] += f32::from(pixel[1]) / 255.0;
            vector[(bucket + 2) % self.vector_size] += f32::from(pixel[2]) / 255.0;
        }

        normalize(&mut vector);
        vector
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

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgb};

    use super::ImageEmbedder;

    #[test]
    fn embedder_returns_normalized_configured_vector_size() {
        let image = ImageBuffer::from_pixel(8, 8, Rgb([255, 128, 64]));
        let vector = ImageEmbedder::new("test", 16).encode(&image);
        assert_eq!(vector.len(), 16);
        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.0001);
    }
}
