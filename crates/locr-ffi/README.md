# locr-ffi — the C ABI that makes locr universal

One frozen `extern "C"` surface (`include/locr.h`) + prebuilt `cdylib`/`staticlib`
per platform = every language on earth can run local OCR with zero cloud calls.

## The whole API

```c
const char *locr_version(void);
const char *locr_last_error(void);    /* message for the last failure on this thread */

LocrStatus  locr_image_to_text(const uint8_t *bytes, size_t len, char **out_text);
LocrStatus  locr_image_to_text_ex(const uint8_t *bytes, size_t len, int auto_enhance,
                                  float min_score, uint32_t max_attempts, char **out_text,
                                  float *out_score, char **out_transform, uint32_t *out_attempts);

/* Lines, words, bounding boxes and confidence as JSON (schema version 1). */
LocrStatus  locr_image_to_json(const uint8_t *bytes, size_t len,
                               const char *options_json, char **out_json);
LocrStatus  locr_pixels_to_json(const uint8_t *pixels, size_t len, uint32_t width,
                                uint32_t height, const char *options_json, char **out_json);

void        locr_free_text(char *text);
```

`options_json` is `NULL` or an object with any of
`{"allowed_chars":"0123456789","beam_width":5,"auto_enhance":true,"min_score":0.55,"max_attempts":7}`.
Unknown keys are rejected, so typos fail loudly instead of being ignored.

Shape of the JSON result (values are illustrative):

```json
{"version":1,"text":"HELLO LOCR","score":0.93,"confidence":0.97,"transform":"none",
 "attempts":1,"width":400,"height":120,
 "lines":[{"text":"HELLO LOCR","confidence":0.97,"bbox":{"x":18,"y":38,"width":66,"height":15},
           "words":[{"text":"HELLO","confidence":0.98,"bbox":{"x":18,"y":38,"width":36,"height":15}},
                    {"text":"LOCR","confidence":0.96,"bbox":{"x":58,"y":38,"width":26,"height":15}}]}]}
```

Boxes are pixels of the image as displayed (EXIF orientation applied).

## Errors

Every failing call returns a negative `LocrStatus` and leaves a message for
`locr_last_error()`. Panics inside locr never unwind into your process: they
become `LOCR_INTERNAL`.

```c
if (locr_image_to_json(bytes, len, NULL, &json) != LOCR_OK) {
    fprintf(stderr, "locr: %s\n", locr_last_error());
}
```

## Consume it from anywhere

### C# / .NET (P/Invoke)

```csharp
[DllImport("locr")] static extern int locr_image_to_text(byte[] bytes, nuint len, out IntPtr text);
[DllImport("locr")] static extern int locr_image_to_json(byte[] bytes, nuint len, string? options, out IntPtr json);
[DllImport("locr")] static extern IntPtr locr_last_error();
[DllImport("locr")] static extern void locr_free_text(IntPtr text);

var bytes = File.ReadAllBytes("invoice.png");
if (locr_image_to_json(bytes, (nuint)bytes.Length, null, out var p) == 0) {
    using var doc = JsonDocument.Parse(Marshal.PtrToStringUTF8(p)!);
    locr_free_text(p);
} else {
    Console.Error.WriteLine(Marshal.PtrToStringUTF8(locr_last_error()));
}
```

### Java (Panama / FFM)

```java
Linker linker = Linker.nativeLinker();
SymbolLookup locr = SymbolLookup.libraryLookup("locr", Arena.global());
// bind locr_image_to_json / locr_free_text with MethodHandles
```

### Go (cgo)

```go
// #cgo LDFLAGS: -llocr
// #include "locr.h"
import "C"

var out *C.char
status := C.locr_image_to_json((*C.uint8_t)(&data[0]), C.size_t(len(data)), nil, &out)
defer C.locr_free_text(out)
```

### C / C++

See [`examples/c/locr_cli.c`](examples/c/locr_cli.c):

```bash
cargo build -p locr-ffi --release
cc -I crates/locr-ffi/include crates/locr-ffi/examples/c/locr_cli.c -L target/release -llocr -o locr_cli
LD_LIBRARY_PATH=target/release ./locr_cli invoice.png
```

## Build

```bash
cargo build -p locr-ffi --release
# -> target/release/locr.dll | liblocr.so | liblocr.dylib (+ static .a/.lib)
```

The models are embedded by default. To ship them separately (smaller library,
or distro packaging), build with `--no-default-features` and point
`LOCR_MODELS_DIR` at a directory holding `text-detection.rten` and
`text-recognition.rten`.

Prebuilt binaries for Linux/macOS/Windows (x64 + ARM64) ship on every release
and as CI artifacts.

## ABI stability

This surface is frozen under semver. New functions may be added; existing
signatures never change without a major version bump.
