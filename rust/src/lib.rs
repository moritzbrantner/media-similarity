use std::path::Path;

pub mod api;
pub mod config;
pub mod embedder;
pub mod hashing;
pub mod image_io;
pub mod indexer;
pub mod media;
pub mod models;
pub mod qdrant;
pub mod search;
pub mod sources;
pub mod thumbnails;

use image::{DynamicImage, ImageDecoder, ImageReader};
use image_analysis_core::OwnedImage;
use image_analysis_io::{write_image_with_format, ImageFileFormat};
use image_analysis_processing::resize_nearest;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use vector_analysis_core::cosine_similarity as rust_cosine_similarity;

fn py_value_error(error: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(error.to_string())
}

fn thumbnail_dimensions(
    width: u32,
    height: u32,
    max_width: u32,
    max_height: u32,
) -> PyResult<(u32, u32)> {
    if width == 0 || height == 0 || max_width == 0 || max_height == 0 {
        return Err(PyValueError::new_err(
            "image and thumbnail dimensions must be non-zero",
        ));
    }

    if width <= max_width && height <= max_height {
        return Ok((width, height));
    }

    let width_scale = max_width as f64 / width as f64;
    let height_scale = max_height as f64 / height as f64;
    let scale = width_scale.min(height_scale);
    let resized_width = ((width as f64 * scale).round() as u32).max(1);
    let resized_height = ((height as f64 * scale).round() as u32).max(1);
    Ok((resized_width, resized_height))
}

fn parse_hex_hash(value: &str) -> PyResult<u64> {
    if value.len() != 16 {
        return Err(PyValueError::new_err(
            "image hash must be a 16 character hexadecimal string",
        ));
    }
    u64::from_str_radix(value, 16)
        .map_err(|_| PyValueError::new_err("image hash must be a 16 character hexadecimal string"))
}

fn validate_rgb_buffer(rgb: &[u8], width: u32, height: u32) -> PyResult<()> {
    if width == 0 || height == 0 {
        return Err(PyValueError::new_err("image dimensions must be non-zero"));
    }
    let expected = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(3))
        .ok_or_else(|| PyValueError::new_err("image dimensions overflow RGB buffer length"))?
        as usize;
    if rgb.len() != expected {
        return Err(PyValueError::new_err(format!(
            "RGB buffer length must be {expected} bytes for {width}x{height} image"
        )));
    }
    Ok(())
}

fn validate_luma_buffer(luma: &[u8], width: u32, height: u32) -> PyResult<()> {
    if width == 0 || height == 0 {
        return Err(PyValueError::new_err("image dimensions must be non-zero"));
    }
    let expected = width
        .checked_mul(height)
        .ok_or_else(|| PyValueError::new_err("image dimensions overflow luma buffer length"))?
        as usize;
    if luma.len() != expected {
        return Err(PyValueError::new_err(format!(
            "luma buffer length must be {expected} bytes for {width}x{height} image"
        )));
    }
    Ok(())
}

fn load_dynamic_image(path: &Path) -> PyResult<DynamicImage> {
    let mut decoder = ImageReader::open(path)
        .map_err(py_value_error)?
        .into_decoder()
        .map_err(py_value_error)?;
    let orientation = decoder.orientation().map_err(py_value_error)?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(py_value_error)?;
    image.apply_orientation(orientation);
    Ok(image)
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

#[pyfunction]
fn load_image_rgb<'py>(py: Python<'py>, path: &str) -> PyResult<(u32, u32, Bound<'py, PyBytes>)> {
    let image = load_dynamic_image(Path::new(path))?.to_rgb8();
    let width = image.width();
    let height = image.height();
    let rgb = image.into_raw();
    Ok((width, height, PyBytes::new(py, &rgb)))
}

#[pyfunction]
fn write_thumbnail_rgb(
    rgb: &[u8],
    width: u32,
    height: u32,
    output_path: &str,
    max_width: u32,
    max_height: u32,
) -> PyResult<()> {
    validate_rgb_buffer(rgb, width, height)?;
    let source = OwnedImage::new_rgb(width, height, rgb.to_vec()).map_err(py_value_error)?;
    let (thumbnail_width, thumbnail_height) =
        thumbnail_dimensions(width, height, max_width, max_height)?;
    let thumbnail = if thumbnail_width == width && thumbnail_height == height {
        source
    } else {
        resize_nearest(&source.as_view(), thumbnail_width, thumbnail_height)
            .map_err(py_value_error)?
    };

    write_image_with_format(Path::new(output_path), &thumbnail, ImageFileFormat::Jpeg)
        .map_err(py_value_error)
}

#[pyfunction]
fn phash_luma(luma: &[u8], width: u32, height: u32, hash_size: u32) -> PyResult<String> {
    validate_luma_buffer(luma, width, height)?;
    if hash_size < 2 {
        return Err(PyValueError::new_err(
            "Hash size must be greater than or equal to 2",
        ));
    }
    if hash_size > width || hash_size > height {
        return Err(PyValueError::new_err(
            "hash size must not exceed luma image dimensions",
        ));
    }
    if hash_size > 8 {
        return Err(PyValueError::new_err(
            "hash size must be 8 or smaller for hexadecimal u64 output",
        ));
    }

    let pixels = luma
        .iter()
        .map(|pixel| f64::from(*pixel))
        .collect::<Vec<_>>();
    let rows = height as usize;
    let cols = width as usize;
    let hash_size = hash_size as usize;
    let dct_rows = dct_axis(&pixels, rows, cols, 0);
    let dct = dct_axis(&dct_rows, rows, cols, 1);

    let mut low_frequency = Vec::with_capacity(hash_size * hash_size);
    for y in 0..hash_size {
        for x in 0..hash_size {
            let value = dct[y * cols + x];
            low_frequency.push(if value.abs() < 1.0e-6 { 0.0 } else { value });
        }
    }
    let mut sorted_low_frequency = low_frequency.clone();
    let threshold = median(&mut sorted_low_frequency);
    let bits = low_frequency
        .into_iter()
        .map(|value| value > threshold)
        .collect::<Vec<_>>();
    Ok(binary_hash_to_hex(&bits))
}

#[pyfunction]
fn hash_distance(left: &str, right: &str) -> PyResult<u32> {
    let left = parse_hex_hash(left)?;
    let right = parse_hex_hash(right)?;
    Ok((left ^ right).count_ones())
}

#[pyfunction]
fn cosine_similarity(left: Vec<f32>, right: Vec<f32>) -> PyResult<f32> {
    rust_cosine_similarity(&left, &right).map_err(py_value_error)
}

#[pymodule]
fn _rust(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add_function(wrap_pyfunction!(load_image_rgb, module)?)?;
    module.add_function(wrap_pyfunction!(write_thumbnail_rgb, module)?)?;
    module.add_function(wrap_pyfunction!(phash_luma, module)?)?;
    module.add_function(wrap_pyfunction!(hash_distance, module)?)?;
    module.add_function(wrap_pyfunction!(cosine_similarity, module)?)?;
    Ok(())
}
