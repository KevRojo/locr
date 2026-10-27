//! WebAssembly build of locr, consumed by the `locr` npm package.
//!
//! Uses the same pure-Rust [ocrs](https://github.com/robertknight/ocrs) engine as the native
//! core. No network calls, no cloud APIs. Results use the versioned JSON schema shared with
//! the C ABI, CLI and Python bindings; the JS wrapper parses them.

use std::sync::{Arc, Mutex};

use locr_core::{image_from_pixels, Locr, LocrError, OcrOptions, OcrsEngine};
use wasm_bindgen::prelude::*;

/// The active engine: bundled models on first use, or the ones from `loadModels`.
static ENGINE: Mutex<Option<Arc<Locr<OcrsEngine>>>> = Mutex::new(None);

fn js(err: LocrError) -> JsError {
    JsError::new(&err.to_string())
}

fn engine() -> Result<Arc<Locr<OcrsEngine>>, JsError> {
    let mut slot = ENGINE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(engine) = slot.as_ref() {
        return Ok(engine.clone());
    }
    let engine = OcrsEngine::new().map_err(|err| match err {
        LocrError::ModelsNotFound => JsError::new(
            "this locr build has no bundled models: call loadModels(detection, recognition) first",
        ),
        other => js(other),
    })?;
    let engine = Arc::new(Locr::new(engine));
    *slot = Some(engine.clone());
    Ok(engine)
}

fn options(json: Option<String>) -> Result<OcrOptions, JsError> {
    OcrOptions::from_json(json.as_deref().unwrap_or("")).map_err(js)
}

/// Library version.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").into()
}

/// Whether the OCR models are embedded in this build. If not, call
/// `loadModels` before reading images.
#[wasm_bindgen(js_name = hasBundledModels)]
pub fn has_bundled_models() -> bool {
    cfg!(feature = "bundle-models")
}

/// Use these `.onnx` or `.rten` models (e.g. fetched once and cached in IndexedDB)
/// instead of the bundled ones.
#[wasm_bindgen(js_name = loadModels)]
pub fn load_models(detection: Vec<u8>, recognition: Vec<u8>) -> Result<(), JsError> {
    let engine = OcrsEngine::from_model_bytes(detection, recognition).map_err(js)?;
    *ENGINE.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(Locr::new(engine)));
    Ok(())
}

/// Extract text from encoded image bytes (PNG, JPEG, WEBP, BMP, TIFF, GIF...).
#[wasm_bindgen(js_name = imageToText)]
pub fn image_to_text(image_bytes: &[u8]) -> Result<String, JsError> {
    engine()?.image_to_text(image_bytes).map_err(js)
}

/// Full result (lines, words, boxes, confidence) for encoded image bytes, as JSON.
///
/// `options_json` takes `allowed_chars`, `beam_width`, `auto_enhance`,
/// `min_score` and `max_attempts`.
#[wasm_bindgen(js_name = imageToDataJson)]
pub fn image_to_data_json(
    image_bytes: &[u8],
    options_json: Option<String>,
) -> Result<String, JsError> {
    let options = options(options_json)?;
    Ok(engine()?
        .image_to_data(image_bytes, &options)
        .map_err(js)?
        .to_json())
}

/// Same as `imageToDataJson` for raw 8-bit pixels (grey, RGB or RGBA, e.g.
/// canvas `ImageData.data`).
#[wasm_bindgen(js_name = pixelsToDataJson)]
pub fn pixels_to_data_json(
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    options_json: Option<String>,
) -> Result<String, JsError> {
    let options = options(options_json)?;
    let image = image_from_pixels(width, height, pixels).map_err(js)?;
    Ok(engine()?
        .recognize_with(&image, &options)
        .map_err(js)?
        .to_json())
}

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}
