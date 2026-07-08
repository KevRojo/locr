/*
 * locr.h — Stable C ABI for locr: universal local OCR.
 *
 * This header is the standard. Any language with an FFI can consume it:
 * C, C++, C#/.NET (P/Invoke), Java (JNI/Panama), Go (cgo), Swift, Ruby,
 * PHP, Zig, and more.
 *
 * Contract:
 *   - Thread-safe.
 *   - Strings returned by locr must be freed with locr_free_text().
 *   - Return codes: 0 = OK, negative = error.
 *
 * License: MIT. https://github.com/KevRojo/locr
 */

#ifndef LOCR_H
#define LOCR_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Status codes returned by locr functions. */
typedef enum LocrStatus {
    LOCR_OK = 0,
    LOCR_INVALID_INPUT = -1,   /* null pointer or zero length      */
    LOCR_DECODE_ERROR = -2,    /* image bytes could not be decoded */
    LOCR_ENGINE_ERROR = -3,    /* OCR engine failed                */
    LOCR_ENGINE_NOT_FOUND = -4,/* no OCR engine on this system     */
    LOCR_INTERNAL = -5         /* internal error                   */
} LocrStatus;

/*
 * Library version as a static NUL-terminated string (e.g. "0.1.0").
 * Do NOT free the returned pointer.
 */
const char *locr_version(void);

/*
 * Extract text from encoded image bytes (PNG, JPEG, WEBP, BMP, TIFF...).
 *
 * On success (LOCR_OK), *out_text points to a NUL-terminated UTF-8 string
 * owned by locr; release it with locr_free_text(). On failure, *out_text
 * is null.
 */
LocrStatus locr_image_to_text(const uint8_t *bytes,
                              size_t len,
                              char **out_text);

/* Free a string previously returned by locr. Passing null is a no-op. */
void locr_free_text(char *text);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* LOCR_H */
