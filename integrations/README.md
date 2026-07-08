# `locr` integrations — Google, Microsoft, and beyond

This folder contains thin, user-facing integration projects that embed the `locr` local OCR engine into the ecosystems where it can replace paid cloud OCR APIs.

> **Status:** scaffolded. The engine is still using the temporary `tesseract.js` / Tesseract CLI bridges. Once `ocrs` is wired into `locr-core` and `locr-wasm`, the WASM artifact produced by `crates/locr-wasm` will be dropped into each integration and the placeholders will be replaced with real OCR calls.

## Projects

| Project | Target ecosystem | How it uses `locr` | Status |
|---------|------------------|--------------------|--------|
| `microsoft-office-addin` | Word, Excel, PowerPoint, Outlook | Office.js task pane + WASM (`locr.wasm`) | scaffolded |
| `google-workspace-addon` | Docs, Sheets, Slides | Google Apps Script + HTML service + WASM embedded as base64 | scaffolded |
| `vscode-extension` | Visual Studio Code | Node extension + WASM/N-API | scaffolded |

## The golden rule

Every integration uses the **same core** (`locr.wasm`) and exposes the **same 3-line API**:

```js
import { imageToText } from 'locr';
const text = await imageToText(imageBytes);
```

No cloud keys, no subscription, no telemetry.

## Next steps

1. Wire `ocrs` into `locr-core` so `locr-wasm` produces a real OCR `.wasm`.
2. Copy `locr.wasm` (or `index.js` from `packages/locr-js`) into each integration.
3. Replace the placeholder `// TODO: run OCR` comments with real calls.
4. Validate each manifest against its respective store (Microsoft AppSource, Google Workspace Marketplace, VS Code Marketplace).
