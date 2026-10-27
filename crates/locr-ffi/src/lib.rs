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
//! - Panics never cross the boundary: they become `LOCR_INTERNAL`.
//! - `locr_last_error` describes the most recent failure on the calling thread.

use locr_core::{self, image_from_pixels, EnhanceOptions, LocrError, OcrOptions};
use std::cell::RefCell;
use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;

/// Status codes returned by locr FFI functions.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocrStatus {
    /// Success.
    Ok = 0,
    /// Input pointer was null, length was zero, or options were invalid.
    InvalidInput = -1,
    /// Image bytes could not be decoded.
    DecodeError = -2,
    /// The OCR engine failed.
    EngineError = -3,
    /// No OCR engine is available on this system.
    EngineNotFound = -4,
    /// Internal error (e.g. interior NUL in output, or a caught panic).
    Internal = -5,
}

/// A failed call: the status to return and the message for `locr_last_error`.
struct Failure(LocrStatus, String);

impl From<LocrError> for Failure {
    fn from(err: LocrError) -> Self {
        let status = match &err {
            LocrError::DecodeError(_) => LocrStatus::DecodeError,
            LocrError::ModelsNotFound | LocrError::TesseractNotFound => LocrStatus::EngineNotFound,
            LocrError::InvalidInput(_) => LocrStatus::InvalidInput,
            _ => LocrStatus::EngineError,
        };
        Failure(status, err.to_string())
    }
}

fn invalid(message: &str) -> Failure {
    Failure(LocrStatus::InvalidInput, message.to_string())
}

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

/// Run an FFI body: reset the thread's last error, turn panics into
/// `Internal`, and record the message of any failure.
fn guard(body: impl FnOnce() -> Result<(), Failure>) -> LocrStatus {
    LAST_ERROR.with(|e| *e.borrow_mut() = None);
    let (status, message) = match catch_unwind(AssertUnwindSafe(body)) {
        Ok(Ok(())) => return LocrStatus::Ok,
        Ok(Err(Failure(status, message))) => (status, message),
        Err(panic) => {
            let detail = panic
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
                .unwrap_or("unknown panic");
            (LocrStatus::Internal, format!("internal error: {detail}"))
        }
    };
    let message = CString::new(message.replace('\0', " ")).unwrap_or_default();
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(message));
    status
}

/// Null `*out` and return it as a writable slot.
///
/// # Safety
/// `out` must be null or valid for writes.
unsafe fn out_slot<'a, T>(out: *mut *mut T, name: &str) -> Result<&'a mut *mut T, Failure> {
    // SAFETY: the caller guarantees `out` is null or writable.
    let slot = unsafe { out.as_mut() }.ok_or_else(|| invalid(&format!("{name} is null")))?;
    *slot = ptr::null_mut();
    Ok(slot)
}

/// # Safety
/// `bytes` must be null or point to `len` readable bytes.
unsafe fn input_bytes<'a>(bytes: *const u8, len: usize) -> Result<&'a [u8], Failure> {
    if bytes.is_null() || len == 0 {
        return Err(invalid("input bytes are null or empty"));
    }
    // SAFETY: non-null, and the caller guarantees `len` readable bytes.
    Ok(unsafe { std::slice::from_raw_parts(bytes, len) })
}

/// # Safety
/// `json` must be null or a NUL-terminated string.
unsafe fn options(json: *const c_char) -> Result<OcrOptions, Failure> {
    if json.is_null() {
        return Ok(OcrOptions::default());
    }
    // SAFETY: non-null, and the caller guarantees NUL termination.
    let json = unsafe { CStr::from_ptr(json) }
        .to_str()
        .map_err(|_| invalid("options_json is not valid UTF-8"))?;
    Ok(OcrOptions::from_json(json)?)
}

fn c_string(s: String) -> Result<CString, Failure> {
    CString::new(s).map_err(|_| Failure(LocrStatus::Internal, "output contained a NUL byte".into()))
}

/// Returns the locr library version as a static NUL-terminated string.
/// The returned pointer is static; do NOT free it.
#[no_mangle]
pub extern "C" fn locr_version() -> *const c_char {
    concat!(env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

/// Description of the most recent error on the calling thread, or null if
/// the last locr call on this thread succeeded.
///
/// The string is owned by locr and valid until the next locr call on the
/// same thread. Do NOT free it.
#[no_mangle]
pub extern "C" fn locr_last_error() -> *const c_char {
    LAST_ERROR.with(|e| e.borrow().as_ref().map_or(ptr::null(), |msg| msg.as_ptr()))
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
    guard(|| {
        // SAFETY: forwarded from this function's contract.
        let out_text = unsafe { out_slot(out_text, "out_text") }?;
        let data = unsafe { input_bytes(bytes, len) }?;
        let text = locr_core::shared()?.image_to_text(data)?;
        *out_text = c_string(text)?.into_raw();
        Ok(())
    })
}

/// OCR with quality score + optional auto-enhance (superpower).
///
/// If `auto_enhance` is non-zero and the first pass scores below `min_score`,
/// locr retries with contrast/brightness/saturation/grayscale/invert and keeps
/// the best result.
///
/// On success:
/// - `*out_text` is a NUL-terminated UTF-8 string (free with `locr_free_text`)
/// - `*out_score` is in `[0.0, 1.0]`
/// - `*out_transform` is a static-ish diagnostic string (free with `locr_free_text`)
/// - `*out_attempts` is the number of OCR passes used
///
/// Pass null for any out-pointer you don't care about (except `out_text` which
/// is required). On failure, `*out_text` and `*out_transform` are null.
///
/// # Safety
/// Same contract as `locr_image_to_text`; non-null optional out-pointers must
/// be valid for writes.
#[no_mangle]
pub unsafe extern "C" fn locr_image_to_text_ex(
    bytes: *const u8,
    len: usize,
    auto_enhance: i32,
    min_score: f32,
    max_attempts: u32,
    out_text: *mut *mut c_char,
    out_score: *mut f32,
    out_transform: *mut *mut c_char,
    out_attempts: *mut u32,
) -> LocrStatus {
    guard(|| {
        // SAFETY: forwarded from this function's contract.
        let out_text = unsafe { out_slot(out_text, "out_text") }?;
        let out_transform = if out_transform.is_null() {
            None
        } else {
            Some(unsafe { out_slot(out_transform, "out_transform") }?)
        };
        let data = unsafe { input_bytes(bytes, len) }?;
        let locr = locr_core::shared()?;

        let result = if auto_enhance != 0 {
            let defaults = EnhanceOptions::default();
            locr.image_to_text_auto(
                data,
                EnhanceOptions {
                    min_score: if min_score <= 0.0 { defaults.min_score } else { min_score },
                    max_attempts: if max_attempts == 0 { defaults.max_attempts } else { max_attempts },
                },
            )?
        } else {
            locr.image_to_text_scored(data)?
        };

        // Convert everything before writing, so a failure leaks nothing.
        let text = c_string(result.text)?;
        let transform = match out_transform {
            Some(_) => Some(c_string(result.transform.to_string())?),
            None => None,
        };
        *out_text = text.into_raw();
        if let (Some(slot), Some(transform)) = (out_transform, transform) {
            *slot = transform.into_raw();
        }
        // SAFETY: optional out-pointers are null or writable per the contract.
        if let Some(score) = unsafe { out_score.as_mut() } {
            *score = result.score;
        }
        if let Some(attempts) = unsafe { out_attempts.as_mut() } {
            *attempts = result.attempts;
        }
        Ok(())
    })
}

/// Full OCR result as JSON (schema version 1): text, score, confidence,
/// transform, attempts, width, height, and lines with words, bounding boxes
/// and confidence.
///
/// `options_json` may be null (defaults) or a JSON object such as
/// `{"allowed_chars":"0123456789","beam_width":5,"auto_enhance":true,
/// "min_score":0.55,"max_attempts":7}`. Unknown keys are rejected with
/// `LOCR_INVALID_INPUT`.
///
/// On success `*out_json` is a NUL-terminated UTF-8 string; free it with
/// `locr_free_text`. On failure it is null.
///
/// # Safety
/// `bytes` must point to `len` readable bytes, `options_json` must be null
/// or NUL-terminated, and `out_json` must be a valid pointer to a `char*`.
#[no_mangle]
pub unsafe extern "C" fn locr_image_to_json(
    bytes: *const u8,
    len: usize,
    options_json: *const c_char,
    out_json: *mut *mut c_char,
) -> LocrStatus {
    guard(|| {
        // SAFETY: forwarded from this function's contract.
        let out_json = unsafe { out_slot(out_json, "out_json") }?;
        let data = unsafe { input_bytes(bytes, len) }?;
        let options = unsafe { options(options_json) }?;
        let result = locr_core::shared()?.image_to_data(data, &options)?;
        *out_json = c_string(result.to_json())?.into_raw();
        Ok(())
    })
}

/// Same as `locr_image_to_json`, for raw 8-bit pixels: tightly packed,
/// row-major, no row padding. The channel count is inferred from `len`:
/// `width*height` (grey), `*3` (RGB) or `*4` (RGBA).
///
/// # Safety
/// `pixels` must point to `len` readable bytes; see `locr_image_to_json`
/// for the other arguments.
#[no_mangle]
pub unsafe extern "C" fn locr_pixels_to_json(
    pixels: *const u8,
    len: usize,
    width: u32,
    height: u32,
    options_json: *const c_char,
    out_json: *mut *mut c_char,
) -> LocrStatus {
    guard(|| {
        // SAFETY: forwarded from this function's contract.
        let out_json = unsafe { out_slot(out_json, "out_json") }?;
        let data = unsafe { input_bytes(pixels, len) }?;
        let options = unsafe { options(options_json) }?;
        let image = image_from_pixels(width, height, data.to_vec())?;
        let result = locr_core::shared()?.recognize_with(&image, &options)?;
        *out_json = c_string(result.to_json())?.into_raw();
        Ok(())
    })
}

/// Free a string previously returned by locr.
///
/// # Safety
/// `text` must be a pointer returned by a locr function (or null, which is a no-op).
#[no_mangle]
pub unsafe extern "C" fn locr_free_text(text: *mut c_char) {
    if !text.is_null() {
        // SAFETY: the caller guarantees `text` came from `CString::into_raw` in locr.
        drop(unsafe { CString::from_raw(text) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn last_error() -> Option<String> {
        let p = locr_last_error();
        (!p.is_null()).then(|| unsafe { CStr::from_ptr(p) }.to_str().unwrap().to_string())
    }

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
        assert!(last_error().unwrap().contains("null or empty"));
    }

    #[test]
    fn null_out_pointer_is_rejected() {
        let status = unsafe { locr_image_to_text(b"x".as_ptr(), 1, ptr::null_mut()) };
        assert_eq!(status, LocrStatus::InvalidInput);
        assert!(last_error().unwrap().contains("out_text"));
    }

    #[test]
    fn invalid_options_are_rejected_before_ocr() {
        let mut out: *mut c_char = ptr::null_mut();
        let opts = c"{\"beam\": 3}";
        let status = unsafe { locr_image_to_json(b"x".as_ptr(), 1, opts.as_ptr(), &mut out) };
        assert_eq!(status, LocrStatus::InvalidInput);
        assert!(out.is_null());
        assert!(last_error().unwrap().contains("beam"));
    }

    #[test]
    fn bad_pixel_buffers_are_rejected() {
        let mut out: *mut c_char = ptr::null_mut();
        let pixels = [0u8; 10];
        let status = unsafe { locr_pixels_to_json(pixels.as_ptr(), pixels.len(), 2, 2, ptr::null(), &mut out) };
        assert_eq!(status, LocrStatus::InvalidInput);
        assert!(out.is_null());
    }

    #[test]
    fn success_clears_the_last_error() {
        let mut out: *mut c_char = ptr::null_mut();
        unsafe { locr_image_to_text(ptr::null(), 0, &mut out) };
        assert!(last_error().is_some());
        assert_eq!(guard(|| Ok(())), LocrStatus::Ok);
        assert_eq!(last_error(), None);
    }

    #[test]
    fn panics_become_internal_errors() {
        let status = guard(|| panic!("boom"));
        assert_eq!(status, LocrStatus::Internal);
        assert_eq!(last_error().as_deref(), Some("internal error: boom"));
    }

    #[test]
    fn last_error_is_per_thread() {
        let mut out: *mut c_char = ptr::null_mut();
        unsafe { locr_image_to_text(ptr::null(), 0, &mut out) };
        assert!(last_error().is_some());
        std::thread::spawn(|| assert_eq!(last_error(), None)).join().unwrap();
    }

    #[test]
    fn free_null_is_noop() {
        unsafe { locr_free_text(ptr::null_mut()) };
    }
}
