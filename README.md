# `locr` — Local OCR. Universal. Free. Plug & Play.

> No cloud APIs. No paywalls. No intermediaries. Just text from images, running locally everywhere.

`locr` is an open-source standard for local image-to-text extraction. One core engine compiles to native binaries and WebAssembly, then ships with lightweight wrappers for **npm** and **pip**.

The engine is **100% Rust** ([ocrs](https://github.com/robertknight/ocrs) + [RTen](https://github.com/robertknight/rten)), so it runs on ARM/x64, Linux/macOS/Windows, browsers, and edge without installing native OCR binaries. No Tesseract. No cloud. No phone home.

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
let text = locr_core::default()?.image_to_text(&image_bytes)?;
```

### C / C++ / C# / Go / Java / Swift

Consume the frozen C ABI in `crates/locr-ffi/include/locr.h`. Prebuilt artifacts ship on every release.

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
├── crates/locr-core      # Rust engine (trait + ocrs backend)
├── crates/locr-ffi       # Stable C ABI (cdylib + staticlib)
├── crates/locr-wasm      # wasm32 target
├── packages/locr-js      # npm wrapper
├── packages/locr-py      # pip wrapper (PyO3)
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

## Local-first, always

- Images are processed on-device.
- No network requests at runtime (models bundled at build time).
- No subscription tiers.
- Open standard: anyone can implement the same `locr.h` ABI.

## License

Code: MIT — see [LICENSE](./LICENSE).  
Bundled OCR models: CC-BY-SA 4.0 — see [crates/locr-core/NOTICE-MODELS](./crates/locr-core/NOTICE-MODELS).

Built with 🔥 by KevRojo & the locr community.
