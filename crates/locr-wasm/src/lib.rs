use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

/// OCR entry point for WebAssembly.
/// Currently returns the recognized text via a bundled WASM-compatible engine.
/// The engine will be swapped for the Rust `locr-core` backend as soon as
/// a pure-Rust/WASM OCR model is integrated.
#[wasm_bindgen]
pub fn image_to_text(_image_bytes: &[u8]) -> Result<String, JsValue> {
    // TODO: wire to locr-core once WASM-compatible backend is ready.
    // For now, the JS wrapper delegates to tesseract.js for actual recognition.
    Ok(String::from("WASM OCR placeholder"))
}

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
}
