use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

/// OCR entry point for WebAssembly.
///
/// Uses the same pure-Rust [ocrs](https://github.com/robertknight/ocrs) engine as the native core,
/// with models embedded at build time. No network calls, no cloud APIs.
///
/// The engine is cached after the first successful call (see `locr_core::shared`).
#[wasm_bindgen]
pub fn image_to_text(image_bytes: &[u8]) -> Result<String, JsValue> {
    let locr = locr_core::shared().map_err(|e| JsValue::from_str(&e.to_string()))?;
    let text = locr
        .image_to_text(image_bytes)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    Ok(text)
}

/// OCR with quality score (single pass).
///
/// Returns a JS object: `{ text, score, transform, attempts }`.
#[wasm_bindgen]
pub fn image_to_text_scored(image_bytes: &[u8]) -> Result<JsValue, JsValue> {
    let locr = locr_core::shared().map_err(|e| JsValue::from_str(&e.to_string()))?;
    let result = locr
        .image_to_text_scored(image_bytes)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    result_to_js(result)
}

/// Superpower: OCR + auto-enhance when score is low.
///
/// If the first pass scores below `min_score` (default 0.55), retries with
/// contrast / brightness / saturation / grayscale / invert and keeps the best.
///
/// Returns a JS object: `{ text, score, transform, attempts }`.
#[wasm_bindgen]
pub fn image_to_text_auto(
    image_bytes: &[u8],
    min_score: Option<f32>,
    max_attempts: Option<u32>,
) -> Result<JsValue, JsValue> {
    let locr = locr_core::shared().map_err(|e| JsValue::from_str(&e.to_string()))?;
    let opts = locr_core::EnhanceOptions {
        min_score: min_score.unwrap_or(0.55),
        max_attempts: max_attempts.unwrap_or(7),
    };
    let result = locr
        .image_to_text_auto(image_bytes, opts)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    result_to_js(result)
}

fn result_to_js(result: locr_core::OcrResult) -> Result<JsValue, JsValue> {
    let obj = js_sys::Object::new();
    js_sys::Reflect::set(&obj, &"text".into(), &JsValue::from_str(&result.text))
        .map_err(|e| e)?;
    js_sys::Reflect::set(&obj, &"score".into(), &JsValue::from_f64(result.score as f64))
        .map_err(|e| e)?;
    js_sys::Reflect::set(
        &obj,
        &"transform".into(),
        &JsValue::from_str(&result.transform.as_str()),
    )
    .map_err(|e| e)?;
    js_sys::Reflect::set(
        &obj,
        &"attempts".into(),
        &JsValue::from_f64(result.attempts as f64),
    )
    .map_err(|e| e)?;
    Ok(obj.into())
}

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}
