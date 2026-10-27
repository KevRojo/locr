# `locr` — Local OCR. Universal. Free. Plug & Play.

> No cloud APIs. No paywalls. No intermediaries. Just text from images, running locally everywhere.

`locr` is an open-source standard for local image-to-text extraction. One core engine compiles to native binaries and WebAssembly, then ships as a C ABI, a CLI, and lightweight packages for **npm** and **pip** — all returning the same result schema.

The engine is **100% Rust** ([ocrs](https://github.com/robertknight/ocrs) + [RTen](https://github.com/robertknight/rten)), so it runs on ARM/x64, Linux/macOS/Windows, browsers, and edge without installing native OCR binaries. No cloud. No phone home.

## Status (v0.1.0)

| Surface | Engine | Notes |
|---------|--------|-------|
| **Rust** (`locr-core`) | pure-Rust ocrs + RTen | models embedded at build time, SHA-256 verified |
| **C ABI** (`locr-ffi`) | same | text or full JSON results; panics never cross the boundary |
| **CLI** (`locr-cli`) | same | text, or JSON Lines for scripts |
| **JavaScript** (`npm install locr`) | same, compiled to WASM | Node, Bun, Deno, browsers; nothing is fetched at runtime |
| **Python** (`pip install locr`) | same, via PyO3 (abi3 wheels) | optional `locr[fallback]` uses system Tesseract |

## What you get

- **Text, lines, words and bounding boxes**, with **confidence** per word, line and page taken from the recognition model's character probabilities.
- **Phone photos read upright**: EXIF orientation is applied before OCR, and boxes match what image viewers show.
- **Transparent PNGs just work**: they are flattened onto a background that contrasts with the ink (black text on a transparent background used to come out as black-on-black: no text).
- **`allowedChars`** whitelist (digits for meters, receipts, IDs), **CTC beam search**, and the **auto-enhance superpower**: when the quality score is low, locr retries with contrast / brightness / saturation / grayscale / invert and keeps the best result.
- **One versioned JSON schema** across Rust, C, the CLI, JavaScript and Python.

## Quickstart (3 lines)

### Node.js / Bun / Deno / browsers

```bash
npm install locr
```

```ts
import { imageToText, imageToData } from 'locr';
const text = await imageToText('invoice.png');
console.log(text);

// Lines, words, boxes, confidence. Options: allowedChars, beamWidth, autoEnhance, minScore, maxAttempts.
const { lines } = await imageToData('meter.jpg', { allowedChars: '0123456789.' });
```

Accepts paths and `file:` URLs (Node), URLs (browser), `Uint8Array`/`Buffer`, `ArrayBuffer`, `Blob`/`File`, `ImageData`, and in browsers `<img>`, `<canvas>`, `OffscreenCanvas`, `ImageBitmap` and `<video>`. Bundlers pick up the `.wasm` automatically.

### Python

```bash
pip install locr
```

```python
from locr import image_to_text, image_to_data
text = image_to_text("invoice.png")
print(text)

data = image_to_data("receipt.jpg", allowed_chars="0123456789.,")
for line in data["lines"]:
    print(line["text"], line["bbox"], line["confidence"])
```

Accepts paths, bytes, file objects, PIL images and numpy arrays. OCR runs with the GIL released, so threads (and web servers) run in parallel.

### Command line

```bash
cargo install locr-cli
locr invoice.png                        # text
locr --json scans/*.jpg > results.jsonl # one result object per image
```

### Rust

```toml
[dependencies]
locr-core = "0.1"
```

```rust
use locr_core::{EnhanceOptions, OcrOptions};

// `shared()` caches the engine — use this in hot paths / batch OCR.
let locr = locr_core::shared()?;
let text = locr.image_to_text(&image_bytes)?;

// Lines, words, bounding boxes and confidence.
let result = locr.image_to_data(&image_bytes, &OcrOptions::default())?;
for line in &result.lines {
    println!("{} {:?} {:?}", line.text, line.bbox, line.confidence);
}

// Superpower: quality score + auto-enhance when the score is low.
// If score < 0.55, locr retries with contrast/brightness/saturation/etc.
let result = locr.image_to_text_auto(&image_bytes, EnhanceOptions::default())?;
println!("{} (score={:.2}, via {})", result.text, result.score, result.transform);
```

### C / C++ / C# / Go / Java / Swift

Consume the frozen C ABI in `crates/locr-ffi/include/locr.h`. Release artifacts (library, header, CLI) are built by CI on every tag.

```c
#include "locr.h"
char *json = NULL;
if (locr_image_to_json(bytes, len, "{\"allowed_chars\":\"0123456789\"}", &json) == LOCR_OK) {
    printf("%s\n", json);
    locr_free_text(json);
} else {
    fprintf(stderr, "locr: %s\n", locr_last_error());
}
```

See [`crates/locr-ffi/README.md`](crates/locr-ffi/README.md) for the whole API and C#, Go and Java snippets.

## Result schema (version 1)

```json
{"version": 1, "text": "HELLO LOCR", "score": 0.93, "confidence": 0.97,
 "transform": "none", "attempts": 1, "width": 400, "height": 120,
 "lines": [{"text": "HELLO LOCR", "confidence": 0.97,
            "bbox": {"x": 18, "y": 38, "width": 66, "height": 15},
            "words": [{"text": "HELLO", "confidence": 0.98, "bbox": {"x": 18, "y": 38, "width": 36, "height": 15}},
                      {"text": "LOCR", "confidence": 0.96, "bbox": {"x": 58, "y": 38, "width": 26, "height": 15}}]}]}
```

Values are illustrative. Boxes are in pixels of the image as displayed. `confidence` is the mean character probability (`null` when an engine does not report it); `score` blends it with a text heuristic and drives the auto-enhance loop. The C ABI, CLI, npm and pip packages all emit this schema; new fields may appear within a version, breaking changes bump it.

## Architecture

```
locr/
├── crates/locr-core      # Rust engine (OcrEngine trait, ocrs backend, preprocessing, JSON schema)
├── crates/locr-ffi       # Stable C ABI (cdylib + staticlib) + locr.h
├── crates/locr-cli       # `locr` command-line tool
├── crates/locr-wasm      # wasm32 target used by the npm package
├── packages/locr-js      # npm package (Node + browser entries over locr-wasm)
├── packages/locr-py      # pip package (PyO3, abi3, optional Tesseract fallback)
└── .github/workflows     # cross-platform CI/CD
```

## Supported platforms

| Platform | Native | WASM | npm | pip |
|----------|--------|------|-----|-----|
| Linux x64 | ✅ | ✅ | ✅ | ✅ |
| Linux ARM64 | ✅ | ✅ | ✅ | ✅ |
| macOS x64 | ✅ | ✅ | ✅ | ✅ |
| macOS ARM64 | ✅ | ✅ | ✅ | ✅ |
| Windows x64 | ✅ | ✅ | ✅ | ✅ |
| Browser / Edge | — | ✅ | ✅ | — |

## Local-first guarantees

- Images are processed on-device.
- Rust / C / CLI / WASM / npm runtime paths make **no network requests**: models are bundled and SHA-256 pinned at build time.
- No subscription tiers.
- Open standard: anyone can implement the same `locr.h` ABI and JSON schema.
- Optional Python Tesseract fallback is opt-in (`pip install 'locr[fallback]'`), never a hard dependency.

## Bring your own models

Builds without the `bundle-models` feature (smaller binaries, distro packaging, custom or retrained models) load `text-detection.rten` and `text-recognition.rten` at runtime:

- Rust: `OcrsEngine::from_models_dir(dir)`, `from_model_bytes(..)`, `.with_alphabet(..)` for other character sets
- C ABI, Python and Rust `shared()`: the `LOCR_MODELS_DIR` environment variable
- CLI: `--models DIR`
- JavaScript: `await loadModels(detectionBytes, recognitionBytes)` — e.g. fetched once and cached in IndexedDB

Offline or air-gapped builds can pre-download the models into `crates/locr-core/assets/` or point `LOCR_MODELS_DIR` at them at build time; their SHA-256 is verified either way.

## Current limits (honest)

- Recognition models are Latin/English-oriented today; multi-language models land before ABI 1.0 freeze.
- First-call latency pays for model load; subsequent calls reuse a process-wide engine (`shared()` / OnceLock).
- The WASM build with bundled models is ~16 MB (4.4 MB code + 12 MB models). Pages that care can build without models and `loadModels()` from a cache.
- In JavaScript, OCR runs on the calling thread inside the async API: use `worker_threads` or a Web Worker for heavy workloads.
- Edge runtimes that cannot fetch package assets (e.g. Cloudflare Workers, which import `.wasm` as a module) need their own loader entry; it is not packaged yet.
- Text detection runs at the model's fixed input resolution, so tiny text on very large scans can be missed; crop or tile big pages.

## Development

```bash
cargo test --workspace                 # downloads the real models once (SHA-256 pinned)
cargo test -p locr-core -p locr-ffi -p locr-cli --no-default-features   # offline, synthetic models
```

Offline tests run the whole ocrs pipeline with tiny synthetic models (`crates/locr-core/tests/fixtures/synthetic`, generated by `make_models.py`); the recognition step is also checked against ocrs' own output. See `packages/locr-js` and `packages/locr-py` for the npm and pip workflows.

## License

Code: MIT — see [LICENSE](./LICENSE).  
Bundled OCR models: CC-BY-SA 4.0 — see [crates/locr-core/NOTICE-MODELS](./crates/locr-core/NOTICE-MODELS).

Built with 🔥 by KevRojo & the locr community.
