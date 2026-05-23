use image::{GrayImage, RgbImage};

pub use image_analysis_processing::{
    hash_distance, is_near_duplicate, PERCEPTUAL_HASH_HIGHFREQ_FACTOR as PHASH_HIGHFREQ_FACTOR,
    PERCEPTUAL_HASH_SIZE as PHASH_SIZE,
};

pub fn phash_image(image: &RgbImage) -> String {
    image_analysis_processing::perceptual_hash_rgb(image)
}

pub fn phash_luma(luma: &GrayImage, hash_size: u32) -> String {
    image_analysis_processing::perceptual_hash_luma(luma, hash_size)
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
