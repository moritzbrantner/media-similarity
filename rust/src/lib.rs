use std::path::Path;

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
    module.add_function(wrap_pyfunction!(hash_distance, module)?)?;
    module.add_function(wrap_pyfunction!(cosine_similarity, module)?)?;
    Ok(())
}
