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
 *   - Panics never cross the boundary: they become LOCR_INTERNAL.
 *   - locr_last_error() describes the most recent failure on the calling thread.
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
    LOCR_INVALID_INPUT = -1,   /* null pointer, zero length, bad options */
    LOCR_DECODE_ERROR = -2,    /* image bytes could not be decoded      */
    LOCR_ENGINE_ERROR = -3,    /* OCR engine failed                     */
    LOCR_ENGINE_NOT_FOUND = -4,/* no OCR engine on this system          */
    LOCR_INTERNAL = -5         /* internal error                        */
} LocrStatus;

/*
 * Library version as a static NUL-terminated string (e.g. "0.1.0").
 * Do NOT free the returned pointer.
 */
const char *locr_version(void);

/*
 * Description of the most recent error on the calling thread, or NULL if
 * the last locr call on this thread succeeded. Owned by locr and valid until
 * the next locr call on the same thread. Do NOT free.
 */
const char *locr_last_error(void);

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

/*
 * Superpower: OCR with quality score + optional auto-enhance.
 *
 * If auto_enhance != 0 and the first pass scores below min_score (default
 * 0.55 when min_score <= 0), locr retries with contrast / brightness /
 * saturation / grayscale / invert transforms and keeps the best result.
 *
 * On success:
 *   - *out_text       UTF-8 text (free with locr_free_text)
 *   - *out_score      [0.0, 1.0] quality score (nullable)
 *   - *out_transform  diagnostic string e.g. "contrast:30" (nullable; free)
 *   - *out_attempts   number of OCR passes used (nullable)
 *
 * Pass NULL for any out-pointer you don't need (out_text is required).
 */
LocrStatus locr_image_to_text_ex(const uint8_t *bytes,
                                 size_t len,
                                 int auto_enhance,
                                 float min_score,
                                 uint32_t max_attempts,
                                 char **out_text,
                                 float *out_score,
                                 char **out_transform,
                                 uint32_t *out_attempts);

/*
 * Full result as JSON (schema version 1):
 *
 *   {"version":1, "text":"...", "score":0.93, "confidence":0.97,
 *    "transform":"none", "attempts":1, "width":400, "height":120,
 *    "lines":[{"text":"...", "confidence":0.97,
 *              "bbox":{"x":18,"y":26,"width":197,"height":27},
 *              "words":[{"text":"...", "confidence":0.98, "bbox":{...}}]}]}
 *
 * Boxes are in pixels of the image as displayed (EXIF orientation applied).
 * "confidence" is null when the engine does not report it.
 *
 * options_json may be NULL (defaults) or a JSON object; every key is optional:
 *
 *   {"allowed_chars":"0123456789", "beam_width":5,
 *    "auto_enhance":true, "min_score":0.55, "max_attempts":7}
 *
 * Unknown keys are rejected with LOCR_INVALID_INPUT.
 *
 * On success *out_json is a NUL-terminated UTF-8 string; free it with
 * locr_free_text(). On failure it is null.
 */
LocrStatus locr_image_to_json(const uint8_t *bytes,
                              size_t len,
                              const char *options_json,
                              char **out_json);

/*
 * Same as locr_image_to_json() for raw 8-bit pixels: tightly packed,
 * row-major, no row padding. The channel count is inferred from len:
 * width*height (grey), width*height*3 (RGB) or width*height*4 (RGBA).
 */
LocrStatus locr_pixels_to_json(const uint8_t *pixels,
                               size_t len,
                               uint32_t width,
                               uint32_t height,
                               const char *options_json,
                               char **out_json);

/* Free a string previously returned by locr. Passing null is a no-op. */
void locr_free_text(char *text);

#ifdef __cplusplus
} /* extern "C" */
#endif

#endif /* LOCR_H */
