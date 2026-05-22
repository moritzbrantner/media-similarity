use image::imageops::FilterType;
use image::{DynamicImage, GrayImage, RgbImage};

pub const PHASH_SIZE: u32 = 8;
pub const PHASH_HIGHFREQ_FACTOR: u32 = 4;

pub fn phash_image(image: &RgbImage) -> String {
    let image_size = PHASH_SIZE * PHASH_HIGHFREQ_FACTOR;
    let luma = DynamicImage::ImageRgb8(image.clone())
        .grayscale()
        .resize_exact(image_size, image_size, FilterType::Lanczos3)
        .to_luma8();
    phash_luma(&luma, PHASH_SIZE)
}

pub fn phash_luma(luma: &GrayImage, hash_size: u32) -> String {
    let rows = luma.height() as usize;
    let cols = luma.width() as usize;
    let hash_size = hash_size as usize;
    let pixels = luma
        .pixels()
        .map(|pixel| f64::from(pixel[0]))
        .collect::<Vec<_>>();
    let dct_rows = dct_axis(&pixels, rows, cols, 0);
    let dct = dct_axis(&dct_rows, rows, cols, 1);

    let mut low_frequency = Vec::with_capacity(hash_size * hash_size);
    for y in 0..hash_size {
        for x in 0..hash_size {
            let value = dct[y * cols + x];
            low_frequency.push(if value.abs() < 1.0e-6 { 0.0 } else { value });
        }
    }
    let mut sorted = low_frequency.clone();
    let threshold = median(&mut sorted);
    let bits = low_frequency
        .into_iter()
        .map(|value| value > threshold)
        .collect::<Vec<_>>();
    binary_hash_to_hex(&bits)
}

pub fn hash_distance(left: &str, right: &str) -> Result<u32, String> {
    let left = parse_hex_hash(left)?;
    let right = parse_hex_hash(right)?;
    Ok((left ^ right).count_ones())
}

#[allow(dead_code)]
pub fn is_near_duplicate(left: &str, right: &str, max_distance: u32) -> Result<bool, String> {
    Ok(hash_distance(left, right)? <= max_distance)
}

fn parse_hex_hash(value: &str) -> Result<u64, String> {
    if value.len() != 16 {
        return Err("image hash must be a 16 character hexadecimal string".to_string());
    }
    u64::from_str_radix(value, 16)
        .map_err(|_| "image hash must be a 16 character hexadecimal string".to_string())
}

fn dct_axis(values: &[f64], rows: usize, cols: usize, axis: usize) -> Vec<f64> {
    let mut output = vec![0.0; values.len()];
    if axis == 0 {
        for freq_y in 0..rows {
            for x in 0..cols {
                let mut sum = 0.0;
                for y in 0..rows {
                    let angle = std::f64::consts::PI * freq_y as f64 * (2 * y + 1) as f64
                        / (2 * rows) as f64;
                    sum += values[y * cols + x] * angle.cos();
                }
                output[freq_y * cols + x] = 2.0 * sum;
            }
        }
    } else {
        for y in 0..rows {
            for freq_x in 0..cols {
                let mut sum = 0.0;
                for x in 0..cols {
                    let angle = std::f64::consts::PI * freq_x as f64 * (2 * x + 1) as f64
                        / (2 * cols) as f64;
                    sum += values[y * cols + x] * angle.cos();
                }
                output[y * cols + freq_x] = 2.0 * sum;
            }
        }
    }
    output
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(|left, right| left.total_cmp(right));
    let mid = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[mid - 1] + values[mid]) / 2.0
    } else {
        values[mid]
    }
}

fn binary_hash_to_hex(bits: &[bool]) -> String {
    let mut hash = 0_u64;
    for bit in bits {
        hash <<= 1;
        if *bit {
            hash |= 1;
        }
    }
    let width = bits.len().div_ceil(4);
    format!("{hash:0width$x}")
}

#[cfg(test)]
mod tests {
    use image::{ImageBuffer, Rgb};

    use super::{hash_distance, is_near_duplicate, phash_image};

    #[test]
    fn hash_distance_counts_bits() {
        assert_eq!(
            hash_distance("0000000000000000", "ffffffffffffffff").unwrap(),
            64
        );
        assert!(is_near_duplicate("0000000000000000", "0000000000000001", 1).unwrap());
        assert!(!is_near_duplicate("0000000000000000", "0000000000000001", 0).unwrap());
    }

    #[test]
    fn phash_is_compact_hex() {
        let image = ImageBuffer::from_pixel(32, 32, Rgb([128, 64, 32]));
        assert_eq!(phash_image(&image).len(), 16);
    }
}
