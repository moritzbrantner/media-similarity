use std::fs;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, RgbImage};

pub fn thumbnail_path(thumbnail_dir: &Path, image_id: &str) -> PathBuf {
    thumbnail_dir.join(format!("{image_id}.jpg"))
}

pub fn ensure_thumbnail(
    image: &RgbImage,
    thumbnail_dir: &Path,
    image_id: &str,
    size: (u32, u32),
) -> Result<String, String> {
    fs::create_dir_all(thumbnail_dir).map_err(|error| error.to_string())?;
    let output_path = thumbnail_path(thumbnail_dir, image_id);
    if !output_path.exists() {
        let thumbnail = DynamicImage::ImageRgb8(image.clone()).thumbnail(size.0, size.1);
        let file = fs::File::create(&output_path).map_err(|error| error.to_string())?;
        let mut encoder = JpegEncoder::new_with_quality(file, 85);
        encoder
            .encode_image(&thumbnail.resize(
                thumbnail.width(),
                thumbnail.height(),
                FilterType::Nearest,
            ))
            .map_err(|error| error.to_string())?;
    }
    Ok(format!(
        "/thumbnails/{}",
        output_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
    ))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use image::{ImageBuffer, Rgb};

    use super::ensure_thumbnail;

    #[test]
    fn ensure_thumbnail_creates_jpeg_and_returns_url() {
        let dir = tempfile_dir();
        let image = ImageBuffer::from_pixel(640, 480, Rgb([20, 30, 40]));
        let url = ensure_thumbnail(&image, &dir, "image-1", (320, 320)).unwrap();
        assert_eq!(url, "/thumbnails/image-1.jpg");
        assert!(dir.join("image-1.jpg").exists());
    }

    #[test]
    fn ensure_thumbnail_does_not_overwrite_existing_file() {
        let dir = tempfile_dir();
        fs::write(dir.join("image-1.jpg"), b"existing").unwrap();
        let image = ImageBuffer::from_pixel(640, 480, Rgb([20, 30, 40]));
        ensure_thumbnail(&image, &dir, "image-1", (320, 320)).unwrap();
        assert_eq!(fs::read(dir.join("image-1.jpg")).unwrap(), b"existing");
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("image-sim-rust-thumb-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
