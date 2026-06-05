use std::collections::BTreeSet;
use std::fs::File;
use std::io::{BufReader, Cursor};
use std::path::{Component, Path, PathBuf};

use image::codecs::gif::GifDecoder;
use image::{AnimationDecoder, DynamicImage, ImageFormat, ImageReader, RgbImage};
use uuid::Uuid;
use walkdir::WalkDir;

use crate::config::Settings;
use crate::workers::media::media::{DecodedMedia, MediaFrame, MediaKind};

pub fn iter_image_paths(source_dir: &Path, extensions: &BTreeSet<String>) -> Vec<PathBuf> {
    if !source_dir.exists() {
        return Vec::new();
    }
    let mut paths = WalkDir::new(source_dir)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| !has_hidden_component(entry.path(), source_dir))
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

fn has_hidden_component(path: &Path, root: &Path) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    relative.components().any(|component| match component {
        Component::Normal(name) => name
            .to_str()
            .map(|name| name.starts_with('.') && name != "." && name != "..")
            .unwrap_or(false),
        _ => false,
    })
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

pub fn load_media(path: &Path, settings: &Settings) -> Result<DecodedMedia, String> {
    let format = ImageReader::open(path)
        .map_err(|error| error.to_string())?
        .with_guessed_format()
        .map_err(|error| error.to_string())?
        .format();
    if format == Some(ImageFormat::Gif) {
        let file = File::open(path).map_err(|error| error.to_string())?;
        let decoder = GifDecoder::new(BufReader::new(file)).map_err(|error| error.to_string())?;
        return decode_gif(decoder, settings);
    }

    load_image(path)
        .map(|image| decoded_static_image(image, settings.gif_default_frame_delay_ms))
        .map_err(|error| error.to_string())
}

pub fn load_media_bytes(raw: &[u8], settings: &Settings) -> Result<DecodedMedia, String> {
    let format = ImageReader::new(Cursor::new(raw))
        .with_guessed_format()
        .map_err(|error| error.to_string())?
        .format();
    if format == Some(ImageFormat::Gif) {
        let decoder =
            GifDecoder::new(Cursor::new(raw.to_vec())).map_err(|error| error.to_string())?;
        return decode_gif(decoder, settings);
    }

    load_image_bytes(raw)
        .map(|image| decoded_static_image(image, settings.gif_default_frame_delay_ms))
        .map_err(|error| error.to_string())
}

fn decoded_static_image(image: RgbImage, delay_ms: u32) -> DecodedMedia {
    let frame = MediaFrame {
        image: image.clone(),
        delay_ms,
    };
    DecodedMedia {
        kind: MediaKind::StaticImage,
        width: image.width(),
        height: image.height(),
        frame_count: None,
        duration_ms: None,
        poster: image,
        sampled_frames: vec![frame.clone()],
        preview_frames: vec![frame],
        audio_analysis: None,
    }
}

fn decode_gif<R>(decoder: GifDecoder<R>, settings: &Settings) -> Result<DecodedMedia, String>
where
    R: std::io::BufRead + std::io::Seek,
{
    let mut frames = Vec::new();
    let mut duration_ms = 0_u32;

    for frame in decoder.into_frames().take(settings.gif_max_decode_frames) {
        let frame = frame.map_err(|error| error.to_string())?;
        let delay_ms = normalized_delay_ms(frame.delay(), settings.gif_default_frame_delay_ms);
        duration_ms = duration_ms.saturating_add(delay_ms);
        frames.push(MediaFrame {
            image: DynamicImage::ImageRgba8(frame.into_buffer()).to_rgb8(),
            delay_ms,
        });
    }

    let poster = frames
        .first()
        .map(|frame| frame.image.clone())
        .ok_or_else(|| "GIF does not contain any decodable frames".to_string())?;
    let frame_count = frames.len() as u32;
    let sampled_frames = sample_frames(&frames, settings.gif_sample_frames);
    let preview_frames = sample_frames(&frames, settings.gif_preview_frames);
    Ok(DecodedMedia {
        kind: MediaKind::AnimatedGif,
        width: poster.width(),
        height: poster.height(),
        frame_count: Some(frame_count),
        duration_ms: Some(duration_ms),
        poster,
        sampled_frames,
        preview_frames,
        audio_analysis: None,
    })
}

fn normalized_delay_ms(delay: image::Delay, default_ms: u32) -> u32 {
    let (numerator, denominator) = delay.numer_denom_ms();
    let delay_ms = if denominator == 0 {
        0
    } else {
        ((numerator as f64) / (denominator as f64)).round() as u32
    };
    if delay_ms == 0 {
        default_ms
    } else {
        delay_ms
    }
}

fn sample_frames(frames: &[MediaFrame], limit: usize) -> Vec<MediaFrame> {
    if frames.len() <= limit {
        return frames.to_vec();
    }
    if limit == 1 {
        return vec![frames[0].clone()];
    }
    let last = frames.len() - 1;
    let denominator = limit - 1;
    (0..limit)
        .map(|index| {
            let source_index = (index * last + denominator / 2) / denominator;
            frames[source_index].clone()
        })
        .collect()
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

    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame};
    use image::{ImageBuffer, Rgb};

    use super::{image_id_for_uri, iter_image_paths, load_image, load_media, relative_path};
    use crate::config::parse_extensions;
    use crate::workers::media::media::MediaKind;

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
    fn iter_image_paths_skips_hidden_cache_paths() {
        let temp = tempfile_dir();
        fs::create_dir_all(temp.join(".gs").join("cache")).unwrap();
        fs::create_dir_all(temp.join("Phone").join(".thumbnails")).unwrap();
        fs::write(temp.join("visible.jpg"), b"x").unwrap();
        fs::write(temp.join(".hidden.jpg"), b"x").unwrap();
        fs::write(temp.join(".gs").join("cache").join("ghost.jpg"), b"x").unwrap();
        fs::write(
            temp.join("Phone").join(".thumbnails").join("thumb.jpg"),
            b"x",
        )
        .unwrap();

        let paths = iter_image_paths(&temp, &parse_extensions(".jpg").unwrap());
        let relative = paths
            .iter()
            .map(|path| relative_path(path, &temp))
            .collect::<Vec<_>>();

        assert_eq!(relative, vec!["visible.jpg"]);
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

    #[test]
    fn load_media_returns_static_image_for_png() {
        let temp = tempfile_dir();
        let path = temp.join("sample.png");
        let image = ImageBuffer::from_pixel(10, 12, Rgb([1_u8, 2_u8, 3_u8]));
        image.save(&path).unwrap();

        let loaded = load_media(&path, &crate::config::Settings::default()).unwrap();
        assert_eq!(loaded.kind, MediaKind::StaticImage);
        assert_eq!((loaded.width, loaded.height), (10, 12));
        assert_eq!(loaded.sampled_frames.len(), 1);
        assert_eq!(loaded.preview_frames.len(), 1);
    }

    #[test]
    fn load_media_decodes_gif_frames_and_duration() {
        let temp = tempfile_dir();
        let path = temp.join("sample.gif");
        write_test_gif(&path, &[[10, 20, 30], [80, 20, 30], [10, 90, 30]], 50);

        let loaded = load_media(&path, &crate::config::Settings::default()).unwrap();
        assert_eq!(loaded.kind, MediaKind::AnimatedGif);
        assert_eq!((loaded.width, loaded.height), (10, 12));
        assert_eq!(loaded.frame_count, Some(3));
        assert_eq!(loaded.duration_ms, Some(150));
        assert_eq!(loaded.sampled_frames.len(), 3);
        assert_eq!(loaded.preview_frames.len(), 3);
    }

    fn write_test_gif(path: &std::path::Path, colors: &[[u8; 3]], delay_ms: u32) {
        let file = fs::File::create(path).unwrap();
        let mut encoder = GifEncoder::new(file);
        encoder.set_repeat(Repeat::Infinite).unwrap();
        let frames = colors
            .iter()
            .map(|color| {
                let image = ImageBuffer::from_pixel(10, 12, Rgb(*color));
                Frame::from_parts(
                    image::DynamicImage::ImageRgb8(image).to_rgba8(),
                    0,
                    0,
                    Delay::from_numer_denom_ms(delay_ms, 1),
                )
            })
            .collect::<Vec<_>>();
        encoder.encode_frames(frames).unwrap();
    }

    fn tempfile_dir() -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("image-sim-rust-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }
}
