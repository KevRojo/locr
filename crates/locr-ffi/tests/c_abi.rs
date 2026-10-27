//! End-to-end calls through the C ABI.
//!
//! Without `bundle-models`, the shared engine loads the tiny synthetic models
//! from locr-core's test fixtures (via `LOCR_MODELS_DIR`), so the whole path
//! runs offline. With bundled models the real ones are used.

use std::ffi::{c_char, CStr};
use std::ptr;
use std::sync::Once;

use locr::*;

const HELLO_PNG: &[u8] = include_bytes!("../../locr-core/tests/fixtures/hello.png");

fn init() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        if cfg!(not(feature = "bundle-models")) {
            std::env::set_var(
                "LOCR_MODELS_DIR",
                concat!(
                    env!("CARGO_MANIFEST_DIR"),
                    "/../locr-core/tests/fixtures/synthetic"
                ),
            );
        }
    });
}

/// Take ownership of a locr string.
fn take(p: *mut c_char) -> String {
    assert!(!p.is_null());
    let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_string();
    unsafe { locr_free_text(p) };
    s
}

fn last_error() -> String {
    let p = locr_last_error();
    assert!(!p.is_null(), "expected an error message");
    unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_string()
}

#[test]
fn image_to_text() {
    init();
    let mut out = ptr::null_mut();
    let status = unsafe { locr_image_to_text(HELLO_PNG.as_ptr(), HELLO_PNG.len(), &mut out) };
    assert_eq!(status, LocrStatus::Ok);
    let text = take(out);
    if cfg!(feature = "bundle-models") {
        assert!(
            text.to_uppercase().contains("HELLO") || text.to_uppercase().contains("LOCR"),
            "{text:?}"
        );
    }
    assert!(locr_last_error().is_null());
}

#[test]
fn image_to_json() {
    init();
    let mut out = ptr::null_mut();
    let status =
        unsafe { locr_image_to_json(HELLO_PNG.as_ptr(), HELLO_PNG.len(), ptr::null(), &mut out) };
    assert_eq!(status, LocrStatus::Ok);
    let json: serde_json::Value = serde_json::from_str(&take(out)).unwrap();
    assert_eq!(json["version"], 1);
    assert_eq!(
        (json["width"].as_u64(), json["height"].as_u64()),
        (Some(400), Some(120))
    );
    assert!(json["lines"].is_array());
    assert_eq!(json["transform"], "none");
}

#[test]
fn json_options_are_applied() {
    init();
    let mut out = ptr::null_mut();
    let opts = c"{\"auto_enhance\": true, \"min_score\": 1.5, \"max_attempts\": 2, \"allowed_chars\": \"0123456789\"}";
    let status =
        unsafe { locr_image_to_json(HELLO_PNG.as_ptr(), HELLO_PNG.len(), opts.as_ptr(), &mut out) };
    assert_eq!(status, LocrStatus::Ok);
    let json: serde_json::Value = serde_json::from_str(&take(out)).unwrap();
    assert_eq!(json["attempts"], 2);
    let text = json["text"].as_str().unwrap();
    assert!(
        text.chars()
            .all(|c| c.is_ascii_digit() || c.is_whitespace()),
        "{text:?}"
    );
}

#[test]
fn pixels_to_json() {
    init();
    // A blank white RGBA frame: the pixel path runs, nothing is read.
    let rgba = vec![255u8; 64 * 32 * 4];
    let mut out = ptr::null_mut();
    let status =
        unsafe { locr_pixels_to_json(rgba.as_ptr(), rgba.len(), 64, 32, ptr::null(), &mut out) };
    assert_eq!(status, LocrStatus::Ok);
    let json: serde_json::Value = serde_json::from_str(&take(out)).unwrap();
    assert_eq!(
        (json["width"].as_u64(), json["height"].as_u64()),
        (Some(64), Some(32))
    );
    assert_eq!(json["text"], "");
}

#[test]
fn image_to_text_ex_reports_everything() {
    init();
    let (mut text, mut transform) = (ptr::null_mut(), ptr::null_mut());
    let (mut score, mut attempts) = (-1.0f32, 0u32);
    let status = unsafe {
        locr_image_to_text_ex(
            HELLO_PNG.as_ptr(),
            HELLO_PNG.len(),
            1,
            1.5,
            3,
            &mut text,
            &mut score,
            &mut transform,
            &mut attempts,
        )
    };
    assert_eq!(status, LocrStatus::Ok);
    take(text);
    assert!(!take(transform).is_empty());
    assert!((0.0..=1.0).contains(&score));
    assert_eq!(attempts, 3);

    // Optional outputs may be null.
    let mut text = ptr::null_mut();
    let status = unsafe {
        locr_image_to_text_ex(
            HELLO_PNG.as_ptr(),
            HELLO_PNG.len(),
            0,
            0.0,
            0,
            &mut text,
            ptr::null_mut(),
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    assert_eq!(status, LocrStatus::Ok);
    take(text);
}

#[test]
fn decode_errors_have_messages() {
    init();
    let mut out = ptr::null_mut();
    let junk = b"definitely not an image";
    let status = unsafe { locr_image_to_json(junk.as_ptr(), junk.len(), ptr::null(), &mut out) };
    assert_eq!(status, LocrStatus::DecodeError);
    assert!(out.is_null());
    assert!(last_error().contains("decode"));
}
