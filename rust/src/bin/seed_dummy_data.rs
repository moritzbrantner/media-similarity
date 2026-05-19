use std::env;
use std::path::{Path, PathBuf};

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::webp::WebPEncoder;
use image::{ColorType, ImageEncoder, ImageFormat, Rgb, RgbImage};

const DEFAULT_COLORS: [[u8; 3]; 8] = [
    [220, 68, 55],
    [58, 132, 92],
    [58, 104, 184],
    [232, 176, 59],
    [141, 87, 166],
    [54, 155, 171],
    [226, 116, 65],
    [96, 96, 96],
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir =
        PathBuf::from(env::var("DUMMY_DATA_DIR").unwrap_or_else(|_| "sample-images".to_string()));
    let count = positive_int(env::var("DUMMY_IMAGE_COUNT").ok().as_deref(), Some(8))?;
    let (width, height) =
        image_size(&env::var("DUMMY_IMAGE_SIZE").unwrap_or_else(|_| "640x480".to_string()))?;
    let format =
        image_format(&env::var("DUMMY_IMAGE_FORMAT").unwrap_or_else(|_| "JPEG".to_string()))?;
    let extension = extension(format);

    std::fs::create_dir_all(&output_dir)?;
    for index in 1..=count {
        let path = output_dir.join(format!("dummy-{index:02}.{extension}"));
        let image = make_image(index, width, height);
        save_image(&image, &path, format)?;
    }

    println!("Generated {count} dummy images in {}", output_dir.display());
    Ok(())
}

fn make_image(index: u32, width: u32, height: u32) -> RgbImage {
    let background = DEFAULT_COLORS[((index - 1) as usize) % DEFAULT_COLORS.len()];
    let accent = [
        255 - background[0],
        255 - background[1],
        255 - background[2],
    ];
    let mut image = RgbImage::from_pixel(width, height, Rgb(background));

    let shortest_side = width.min(height);
    let margin = 24_u32
        .max(shortest_side / 12)
        .min(shortest_side.saturating_sub(1) / 2);
    let border_width = 4_u32.max(width.min(height) / 60);
    draw_rect_outline(
        &mut image,
        margin,
        margin,
        width - margin,
        height - margin,
        border_width,
        Rgb(accent),
    );

    let step = 16_i32.max((width.min(height) / 8) as i32);
    for offset in (-(height as i32)..width as i32).step_by(step as usize) {
        let color = Rgb([
            accent[0].wrapping_add((index * 17) as u8),
            accent[1].wrapping_add((index * 17) as u8),
            accent[2].wrapping_add((index * 17) as u8),
        ]);
        draw_line(
            &mut image,
            offset,
            height as i32,
            offset + height as i32,
            0,
            color,
        );
    }

    let radius = width.min(height) / 6;
    let center_x = offset_center(width, radius, (index % 3) as i32 - 1);
    let center_y = offset_center(height, radius, ((index + 1) % 3) as i32 - 1);
    draw_circle(&mut image, center_x, center_y, radius, Rgb(accent));
    image
}

fn offset_center(length: u32, radius: u32, direction: i32) -> u32 {
    let center = length as i32 / 2 + direction * radius as i32 / 2;
    center.clamp(radius as i32, length.saturating_sub(radius + 1) as i32) as u32
}

fn draw_rect_outline(
    image: &mut RgbImage,
    left: u32,
    top: u32,
    right: u32,
    bottom: u32,
    width: u32,
    color: Rgb<u8>,
) {
    for inset in 0..width {
        for x in left + inset..=right.saturating_sub(inset) {
            set_pixel(image, x, top + inset, color);
            set_pixel(image, x, bottom.saturating_sub(inset), color);
        }
        for y in top + inset..=bottom.saturating_sub(inset) {
            set_pixel(image, left + inset, y, color);
            set_pixel(image, right.saturating_sub(inset), y, color);
        }
    }
}

fn draw_line(image: &mut RgbImage, mut x0: i32, mut y0: i32, x1: i32, y1: i32, color: Rgb<u8>) {
    let dx = (x1 - x0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let dy = -(y1 - y0).abs();
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut error = dx + dy;

    loop {
        if x0 >= 0 && y0 >= 0 {
            set_pixel(image, x0 as u32, y0 as u32, color);
        }
        if x0 == x1 && y0 == y1 {
            break;
        }
        let e2 = 2 * error;
        if e2 >= dy {
            error += dy;
            x0 += sx;
        }
        if e2 <= dx {
            error += dx;
            y0 += sy;
        }
    }
}

fn draw_circle(image: &mut RgbImage, center_x: u32, center_y: u32, radius: u32, color: Rgb<u8>) {
    let radius_squared = (radius * radius) as i64;
    let min_x = center_x.saturating_sub(radius);
    let max_x = (center_x + radius).min(image.width().saturating_sub(1));
    let min_y = center_y.saturating_sub(radius);
    let max_y = (center_y + radius).min(image.height().saturating_sub(1));

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let dx = x as i64 - center_x as i64;
            let dy = y as i64 - center_y as i64;
            if dx * dx + dy * dy <= radius_squared {
                set_pixel(image, x, y, color);
            }
        }
    }
}

fn set_pixel(image: &mut RgbImage, x: u32, y: u32, color: Rgb<u8>) {
    if x < image.width() && y < image.height() {
        image.put_pixel(x, y, color);
    }
}

fn save_image(
    image: &RgbImage,
    path: &Path,
    format: ImageFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    let file = std::fs::File::create(path)?;
    match format {
        ImageFormat::Jpeg => JpegEncoder::new_with_quality(file, 90).encode_image(image)?,
        ImageFormat::Png => PngEncoder::new(file).write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ColorType::Rgb8.into(),
        )?,
        ImageFormat::WebP => WebPEncoder::new_lossless(file).write_image(
            image.as_raw(),
            image.width(),
            image.height(),
            ColorType::Rgb8.into(),
        )?,
        _ => unreachable!("unsupported dummy image format"),
    }
    Ok(())
}

fn image_size(value: &str) -> Result<(u32, u32), String> {
    let lower = value.to_ascii_lowercase();
    let Some((raw_width, raw_height)) = lower.split_once('x') else {
        return Err("DUMMY_IMAGE_SIZE must use WIDTHxHEIGHT format".to_string());
    };
    Ok((
        positive_int(Some(raw_width), None)?,
        positive_int(Some(raw_height), None)?,
    ))
}

fn positive_int(value: Option<&str>, default: Option<u32>) -> Result<u32, String> {
    match value.filter(|value| !value.is_empty()) {
        Some(value) => {
            let parsed = value
                .parse::<u32>()
                .map_err(|_| "Expected a positive integer".to_string())?;
            if parsed < 1 {
                Err("Expected a positive integer".to_string())
            } else {
                Ok(parsed)
            }
        }
        None => default.ok_or_else(|| "Expected a positive integer".to_string()),
    }
}

fn image_format(value: &str) -> Result<ImageFormat, String> {
    match value.to_ascii_uppercase().as_str() {
        "JPEG" | "JPG" => Ok(ImageFormat::Jpeg),
        "PNG" => Ok(ImageFormat::Png),
        "WEBP" => Ok(ImageFormat::WebP),
        _ => Err("DUMMY_IMAGE_FORMAT must be JPEG, PNG, or WEBP".to_string()),
    }
}

fn extension(format: ImageFormat) -> &'static str {
    match format {
        ImageFormat::Jpeg => "jpg",
        ImageFormat::Png => "png",
        ImageFormat::WebP => "webp",
        _ => unreachable!("unsupported dummy image format"),
    }
}
