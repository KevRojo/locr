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

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}
