use image::RgbImage;

use crate::media::MediaFrame;

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

    pub fn encode_media(&self, frames: &[MediaFrame], motion_weight: f32) -> Vec<f32> {
        if frames.is_empty() {
            return vec![0.0_f32; self.vector_size];
        }
        if frames.len() == 1 {
            return self.encode(&frames[0].image);
        }

        let content =
            weighted_frame_average(frames, |frame| self.encode(&frame.image), self.vector_size);
        if motion_weight <= 0.0 {
            return content;
        }

        let delta_frames = frame_deltas(frames);
        let motion = weighted_frame_average(
            &delta_frames,
            |frame| self.encode(&frame.image),
            self.vector_size,
        );
        let content_weight = 1.0 - motion_weight.clamp(0.0, 1.0);
        let mut vector = vec![0.0_f32; self.vector_size];
        for index in 0..self.vector_size {
            vector[index] = content[index] * content_weight + motion[index] * motion_weight;
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

fn weighted_frame_average<F>(frames: &[MediaFrame], mut encode: F, vector_size: usize) -> Vec<f32>
where
    F: FnMut(&MediaFrame) -> Vec<f32>,
{
    let mut vector = vec![0.0_f32; vector_size];
    let total_weight = frames
        .iter()
        .map(|frame| frame.delay_ms.max(1) as f32)
        .sum::<f32>();
    if total_weight == 0.0 {
        return vector;
    }

    for frame in frames {
        let weight = frame.delay_ms.max(1) as f32 / total_weight;
        let frame_vector = encode(frame);
        for (index, value) in frame_vector.into_iter().enumerate().take(vector_size) {
            vector[index] += value * weight;
        }
    }
    normalize(&mut vector);
    vector
}

fn frame_deltas(frames: &[MediaFrame]) -> Vec<MediaFrame> {
    frames
        .windows(2)
        .map(|pair| MediaFrame {
            image: delta_image(&pair[0].image, &pair[1].image),
            delay_ms: pair[1].delay_ms,
        })
        .collect()
}

fn delta_image(left: &RgbImage, right: &RgbImage) -> RgbImage {
    let width = left.width().min(right.width());
    let height = left.height().min(right.height());
    let mut image = RgbImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let left_pixel = left.get_pixel(x, y);
            let right_pixel = right.get_pixel(x, y);
            image.put_pixel(
                x,
                y,
                image::Rgb([
                    left_pixel[0].abs_diff(right_pixel[0]),
                    left_pixel[1].abs_diff(right_pixel[1]),
                    left_pixel[2].abs_diff(right_pixel[2]),
                ]),
            );
        }
    }
    image
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgb};

    use super::ImageEmbedder;
    use crate::media::MediaFrame;

    #[test]
    fn embedder_returns_normalized_configured_vector_size() {
        let image = ImageBuffer::from_pixel(8, 8, Rgb([255, 128, 64]));
        let vector = ImageEmbedder::new("test", 16).encode(&image);
        assert_eq!(vector.len(), 16);
        let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.0001);
    }

    #[test]
    fn media_embedder_matches_single_frame_image_embedding() {
        let image = ImageBuffer::from_pixel(8, 8, Rgb([255, 128, 64]));
        let embedder = ImageEmbedder::new("test", 16);
        let image_vector = embedder.encode(&image);
        let media_vector = embedder.encode_media(
            &[MediaFrame {
                image,
                delay_ms: 100,
            }],
            0.2,
        );
        assert_eq!(media_vector, image_vector);
    }

    #[test]
    fn media_embedder_includes_motion_signal() {
        let first = ImageBuffer::from_pixel(8, 8, Rgb([20, 20, 20]));
        let second = ImageBuffer::from_pixel(8, 8, Rgb([220, 20, 20]));
        let third = ImageBuffer::from_pixel(8, 8, Rgb([20, 220, 20]));
        let embedder = ImageEmbedder::new("test", 16);
        let static_vector = embedder.encode_media(
            &[
                MediaFrame {
                    image: first.clone(),
                    delay_ms: 100,
                },
                MediaFrame {
                    image: first,
                    delay_ms: 100,
                },
            ],
            0.2,
        );
        let moving_vector = embedder.encode_media(
            &[
                MediaFrame {
                    image: second,
                    delay_ms: 100,
                },
                MediaFrame {
                    image: third,
                    delay_ms: 100,
                },
            ],
            0.2,
        );
        assert_ne!(static_vector, moving_vector);
    }
}
