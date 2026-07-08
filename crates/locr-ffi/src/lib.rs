//! Stable C ABI for `locr`.
//!
//! This surface is FROZEN under semver: no breaking changes without a major
//! version bump. It is the lingua franca that lets every language on earth
//! (C, C++, C#/.NET, Java, Go, Swift, Ruby, PHP, Zig, ...) consume locr.
//!
//! Contract:
//! - All functions are thread-safe.
//! - Strings returned by locr must be freed with `locr_free_text`.
//! - Return codes: 0 = OK, negative = error (see `LocrStatus`).

use locr_core;
use std::ffi::{c_char, CString};
use std::ptr;

/// Status codes returned by locr FFI functions.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocrStatus {
    /// Success.
    Ok = 0,
    /// Input pointer was null or length was zero.
    InvalidInput = -1,
    /// Image bytes could not be decoded.
    DecodeError = -2,
    /// The OCR engine failed.
    EngineError = -3,
    /// No OCR engine is available on this system.
    EngineNotFound = -4,
    /// Internal error (e.g. interior NUL in output).
    Internal = -5,
}

fn map_init_error(err: locr_core::LocrError) -> LocrStatus {
    match err {
        locr_core::LocrError::ModelsNotFound | locr_core::LocrError::TesseractNotFound => {
            LocrStatus::EngineNotFound
        }
        locr_core::LocrError::DecodeError(_) => LocrStatus::DecodeError,
        _ => LocrStatus::EngineError,
    }
}

fn map_run_error(err: locr_core::LocrError) -> LocrStatus {
    match err {
        locr_core::LocrError::DecodeError(_) => LocrStatus::DecodeError,
        locr_core::LocrError::ModelsNotFound | locr_core::LocrError::TesseractNotFound => {
            LocrStatus::EngineNotFound
        }
        _ => LocrStatus::EngineError,
    }
}

/// Returns the locr library version as a static NUL-terminated string.
/// The returned pointer is static; do NOT free it.
#[no_mangle]
pub extern "C" fn locr_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

/// Extract text from encoded image bytes (PNG, JPEG, WEBP, BMP, TIFF...).
///
/// On success, `*out_text` points to a NUL-terminated UTF-8 string that the
/// caller MUST release with `locr_free_text`. On failure, `*out_text` is set
/// to null and a negative `LocrStatus` is returned.
///
/// # Safety
/// `bytes` must point to `len` readable bytes. `out_text` must be a valid
/// pointer to a `char*`.
#[no_mangle]
pub unsafe extern "C" fn locr_image_to_text(
    bytes: *const u8,
    len: usize,
    out_text: *mut *mut c_char,
) -> LocrStatus {
    if out_text.is_null() {
        return LocrStatus::InvalidInput;
    }
    *out_text = ptr::null_mut();
    if bytes.is_null() || len == 0 {
        return LocrStatus::InvalidInput;
    }

    let data = std::slice::from_raw_parts(bytes, len);
    // Shared engine: models load once, not on every FFI call.
    let locr = match locr_core::shared() {
        Ok(l) => l,
        Err(e) => return map_init_error(e),
    };

    match locr.image_to_text(data) {
        Ok(text) => match CString::new(text) {
            Ok(cstr) => {
                *out_text = cstr.into_raw();
                LocrStatus::Ok
            }
            Err(_) => LocrStatus::Internal,
        },
        Err(e) => map_run_error(e),
    }
}

/// Free a string previously returned by locr.
///
/// # Safety
/// `text` must be a pointer returned by a locr function (or null, which is a no-op).
#[no_mangle]
pub unsafe extern "C" fn locr_free_text(text: *mut c_char) {
    if !text.is_null() {
        drop(CString::from_raw(text));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_is_static_and_nul_terminated() {
        let v = locr_version();
        assert!(!v.is_null());
        let s = unsafe { std::ffi::CStr::from_ptr(v) }.to_str().unwrap();
        assert_eq!(s, env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn null_input_is_rejected() {
        let mut out: *mut c_char = ptr::null_mut();
        let status = unsafe { locr_image_to_text(ptr::null(), 0, &mut out) };
        assert_eq!(status, LocrStatus::InvalidInput);
        assert!(out.is_null());
    }

    #[test]
    fn free_null_is_noop() {
        unsafe { locr_free_text(ptr::null_mut()) };
    }
}
