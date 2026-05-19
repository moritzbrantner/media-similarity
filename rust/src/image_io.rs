use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use image::{ImageReader, RgbImage};
use uuid::Uuid;
use walkdir::WalkDir;

pub fn iter_image_paths(source_dir: &Path, extensions: &BTreeSet<String>) -> Vec<PathBuf> {
    if !source_dir.exists() {
        return Vec::new();
    }
    let mut paths = WalkDir::new(source_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| {
            path.extension()
                .and_then(|extension| extension.to_str())
                .map(|extension| {
                    extensions.contains(&format!(".{}", extension.to_ascii_lowercase()))
                })
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths
}

pub fn load_image(path: &Path) -> Result<RgbImage, image::ImageError> {
    Ok(ImageReader::open(path)?
        .with_guessed_format()?
        .decode()?
        .to_rgb8())
}

pub fn load_image_bytes(raw: &[u8]) -> Result<RgbImage, image::ImageError> {
    Ok(ImageReader::new(std::io::Cursor::new(raw))
        .with_guessed_format()?
        .decode()?
        .to_rgb8())
}

pub fn image_id_for_uri(uri: &str) -> String {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, uri.as_bytes()).to_string()
}

pub fn relative_path(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .and_then(|relative| relative.to_str())
        .map(|relative| relative.replace('\\', "/"))
        .unwrap_or_else(|| {
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string()
        })
}

pub fn dimensions(image: &RgbImage) -> (u32, u32) {
    (image.width(), image.height())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use image::{ImageBuffer, Rgb};

    use super::{image_id_for_uri, iter_image_paths, load_image, relative_path};
    use crate::config::parse_extensions;

    #[test]
    fn iter_image_paths_filters_case_insensitively_and_sorts() {
        let temp = tempfile_dir();
        fs::create_dir_all(temp.join("nested")).unwrap();
        fs::write(temp.join("z.txt"), b"x").unwrap();
        fs::write(temp.join("b.PNG"), b"x").unwrap();
        fs::write(temp.join("a.jpg"), b"x").unwrap();
        fs::write(temp.join("nested").join("c.webp"), b"x").unwrap();

        let paths = iter_image_paths(&temp, &parse_extensions(".jpg,.png,.webp").unwrap());
        let relative = paths
            .iter()
            .map(|path| relative_path(path, &temp))
            .collect::<Vec<_>>();
        assert_eq!(relative, vec!["a.jpg", "b.PNG", "nested/c.webp"]);
    }

    #[test]
    fn image_id_for_uri_is_deterministic() {
        assert_eq!(
            image_id_for_uri("minio://bucket/a.jpg"),
            image_id_for_uri("minio://bucket/a.jpg")
        );
        assert_ne!(
            image_id_for_uri("minio://bucket/a.jpg"),
            image_id_for_uri("minio://bucket/b.jpg")
        );
    }

    #[test]
    fn load_image_returns_rgb() {
        let temp = tempfile_dir();
        let path = temp.join("sample.png");
        let image = ImageBuffer::from_pixel(10, 12, Rgb([1_u8, 2_u8, 3_u8]));
        image.save(&path).unwrap();
        let loaded = load_image(&path).unwrap();
        assert_eq!(loaded.dimensions(), (10, 12));
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("image-sim-rust-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
