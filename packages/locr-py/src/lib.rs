use locr_core::{self, EnhanceOptions};
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::PyDict;

/// Extract text from image bytes using the local OCR engine.
///
/// Uses a process-wide shared engine so models are loaded once.
#[pyfunction]
fn image_to_text_bytes(bytes: &[u8]) -> PyResult<String> {
    let locr = locr_core::shared().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    locr.image_to_text(bytes)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))
}

/// Extract text + quality score (single pass, no enhance loop).
///
/// Returns a dict: ``{"text": str, "score": float, "transform": str, "attempts": int}``
#[pyfunction]
fn image_to_text_scored_bytes(bytes: &[u8], py: Python<'_>) -> PyResult<PyObject> {
    let locr = locr_core::shared().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let result = locr
        .image_to_text_scored(bytes)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    result_to_dict(py, result)
}

/// Superpower: OCR with auto-enhance when score is low.
///
/// If the first pass scores below ``min_score`` (default 0.55), locr tries
/// contrast / brightness / saturation / grayscale / invert transforms and
/// keeps the best scoring result.
///
/// Returns a dict: ``{"text": str, "score": float, "transform": str, "attempts": int}``
#[pyfunction]
#[pyo3(signature = (bytes, min_score=0.55, max_attempts=7))]
fn image_to_text_auto_bytes(
    bytes: &[u8],
    min_score: f32,
    max_attempts: u32,
    py: Python<'_>,
) -> PyResult<PyObject> {
    let locr = locr_core::shared().map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let opts = EnhanceOptions {
        min_score,
        max_attempts,
    };
    let result = locr
        .image_to_text_auto(bytes, opts)
        .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    result_to_dict(py, result)
}

fn result_to_dict(py: Python<'_>, result: locr_core::OcrResult) -> PyResult<PyObject> {
    let dict = PyDict::new(py);
    dict.set_item("text", result.text)?;
    dict.set_item("score", result.score)?;
    dict.set_item("transform", result.transform.as_str())?;
    dict.set_item("attempts", result.attempts)?;
    Ok(dict.into())
}

/// locr - local OCR for Python
#[pymodule]
fn _locr(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(image_to_text_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(image_to_text_scored_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(image_to_text_auto_bytes, m)?)?;
    Ok(())
}
