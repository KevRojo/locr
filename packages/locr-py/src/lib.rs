//! Native extension behind the `locr` Python package.
//!
//! OCR runs with the GIL released, so other Python threads keep running and
//! thread pools get real parallelism.

use locr_core::{self, image_from_pixels, LocrError as CoreError, OcrOptions};
use pyo3::create_exception;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

create_exception!(
    _locr,
    LocrError,
    PyRuntimeError,
    "OCR failed: the image could not be decoded, the models are missing, or the engine errored."
);

fn to_py(err: CoreError) -> PyErr {
    match err {
        CoreError::InvalidInput(msg) => PyValueError::new_err(msg),
        other => LocrError::new_err(other.to_string()),
    }
}

/// Extract text from encoded image bytes using the local OCR engine.
///
/// Uses a process-wide shared engine so models are loaded once.
#[pyfunction]
fn image_to_text_bytes(py: Python<'_>, bytes: &[u8]) -> PyResult<String> {
    py.detach(|| locr_core::shared()?.image_to_text(bytes))
        .map_err(to_py)
}

/// Full result for encoded image bytes, as the versioned JSON schema.
///
/// ``options_json`` takes the keys ``allowed_chars``, ``beam_width``,
/// ``auto_enhance``, ``min_score`` and ``max_attempts``.
#[pyfunction]
#[pyo3(signature = (bytes, options_json="{}"))]
fn image_to_data_json(py: Python<'_>, bytes: &[u8], options_json: &str) -> PyResult<String> {
    let options = OcrOptions::from_json(options_json).map_err(to_py)?;
    py.detach(|| {
        Ok(locr_core::shared()?
            .image_to_data(bytes, &options)?
            .to_json())
    })
    .map_err(to_py)
}

/// Full result for raw 8-bit pixels (row-major; 1, 3 or 4 channels inferred
/// from the buffer length), as the versioned JSON schema.
#[pyfunction]
#[pyo3(signature = (width, height, pixels, options_json="{}"))]
fn pixels_to_data_json(
    py: Python<'_>,
    width: u32,
    height: u32,
    pixels: &[u8],
    options_json: &str,
) -> PyResult<String> {
    let options = OcrOptions::from_json(options_json).map_err(to_py)?;
    py.detach(|| {
        let image = image_from_pixels(width, height, pixels.to_vec())?;
        Ok(locr_core::shared()?
            .recognize_with(&image, &options)?
            .to_json())
    })
    .map_err(to_py)
}

/// locr - local OCR for Python
#[pymodule]
fn _locr(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("LocrError", m.py().get_type::<LocrError>())?;
    m.add_function(wrap_pyfunction!(image_to_text_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(image_to_data_json, m)?)?;
    m.add_function(wrap_pyfunction!(pixels_to_data_json, m)?)?;
    Ok(())
}
