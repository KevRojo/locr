# `locr` — Local OCR. Universal. Free. Plug & Play.

> No cloud APIs. No paywalls. No intermediaries. Just text from images, running locally everywhere.

`locr` is an open-source standard for local image-to-text extraction. One core engine compiles to native binaries and WebAssembly, then ships with lightweight wrappers for **npm** and **pip**.

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
use locr_core::Locr;
let text = Locr::new().image_to_text(&image_bytes)?;
```

## Architecture

```
locr/
├── crates/locr-core      # Rust engine (trait + backends)
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
- No network requests.
- No subscription tiers.
- Training data and models bundled or downloaded once.

## License

MIT — see [LICENSE](./LICENSE).

Built with 🔥 by KevRojo & the locr community.
