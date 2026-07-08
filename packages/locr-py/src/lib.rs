use locr_core;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

/// Extract text from image bytes using the local OCR engine.
#[pyfunction]
fn image_to_text_bytes(bytes: &[u8]) -> PyResult<String> {
    let locr = locr_core::default().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    locr.image_to_text(bytes)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

/// locr - local OCR for Python
#[pymodule]
fn _locr(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(image_to_text_bytes, m)?)?;
    Ok(())
}
