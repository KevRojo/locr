# `locr` — Local OCR. Universal. Free. Plug & Play.

> No cloud APIs. No paywalls. No intermediaries. Just text from images, running locally everywhere.

`locr` is an open-source standard for local image-to-text extraction. One core engine compiles to native binaries and WebAssembly, then ships with lightweight wrappers for **npm** and **pip**.

The engine is **100% Rust** ([ocrs](https://github.com/robertknight/ocrs) + [RTen](https://github.com/robertknight/rten)), so it runs on ARM/x64, Linux/macOS/Windows, browsers, and edge without installing native OCR binaries. No cloud. No phone home.

## Status (v0.1.0)

| Surface | Engine today | Notes |
|---------|--------------|-------|
| **Rust / C ABI / WASM** | pure-Rust ocrs + RTen | models embedded at build time, SHA-256 verified |
| **Python (`pip install locr`)** | pure-Rust via PyO3 when the wheel is present | optional `locr[fallback]` uses system Tesseract |
| **JavaScript (`npm install locr`)** | tesseract.js (temporary) | WASM core ships next — migration in progress |

The C ABI and Rust core are the source of truth. npm is honest about the temporary bridge so nobody gets surprised on day one.

## Quickstart (3 lines)

### Node.js / Bun / Deno

```bash
npm install locr
```

```ts
import { imageToText } from 'locr';
const text = await imageToText('invoice.png');
console.log(text);
```

### Python

```bash
pip install locr
```

```python
from locr import image_to_text
text = image_to_text("invoice.png")
print(text)
```

### Rust

```toml
[dependencies]
locr-core = "0.1"
```

```rust
use locr_core;
// `shared()` caches the engine — use this in hot paths / batch OCR.
let text = locr_core::shared()?.image_to_text(&image_bytes)?;
```

### C / C++ / C# / Go / Java / Swift

Consume the frozen C ABI in `crates/locr-ffi/include/locr.h`. Release artifacts are built by CI on every tag.

```c
#include "locr.h"
char *text = NULL;
if (locr_image_to_text(bytes, len, &text) == LOCR_OK) {
    printf("%s\n", text);
    locr_free_text(text);
}
```

## Architecture

```
locr/
├── crates/locr-core      # Rust engine (trait + ocrs backend, shared() cache)
├── crates/locr-ffi       # Stable C ABI (cdylib + staticlib)
├── crates/locr-wasm      # wasm32 target
├── packages/locr-js      # npm wrapper (tesseract.js bridge → WASM next)
├── packages/locr-py      # pip wrapper (PyO3, optional Tesseract fallback)
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
| Browser / Edge | — | ✅ | ✅* | — |

\* npm currently uses tesseract.js; pure WASM path is next.

## Local-first guarantees

- Images are processed on-device.
- Rust / C / WASM runtime path makes **no network requests** (models bundled + SHA-256 pinned at build time).
- No subscription tiers.
- Open standard: anyone can implement the same `locr.h` ABI.
- Optional Python Tesseract fallback is opt-in (`pip install 'locr[fallback]'`), never a hard dependency.

## Current limits (honest)

- Recognition models are Latin/English-oriented today; multi-language opts land before ABI 1.0 freeze.
- First-call latency pays for model load; subsequent calls reuse a process-wide engine (`shared()` / OnceLock).
- WASM binary with bundled models is large (~10–20MB). CDN + IndexedDB cache is the planned browser path.

## License

Code: MIT — see [LICENSE](./LICENSE).  
Bundled OCR models: CC-BY-SA 4.0 — see [crates/locr-core/NOTICE-MODELS](./crates/locr-core/NOTICE-MODELS).

Built with 🔥 by KevRojo & the locr community.
