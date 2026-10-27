# `locr` integrations — Google, Microsoft, and beyond

This folder contains thin, user-facing integration projects that embed the `locr` local OCR engine into the ecosystems where it can replace paid cloud OCR APIs.

> **Status:** the engine is real. `npm install locr` ships the pure-Rust core as WebAssembly (Node + browser builds), so every integration can run OCR locally. The VS Code extension is wired to it; the Office and Google add-ons still call placeholder functions.

## Projects

| Project | Target ecosystem | How it uses `locr` | Status |
|---------|------------------|--------------------|--------|
| `microsoft-office-addin` | Word, Excel, PowerPoint, Outlook | Office.js task pane + `locr` browser build | scaffolded |
| `google-workspace-addon` | Docs, Sheets, Slides | Google Apps Script + HTML service + WASM embedded as base64 | scaffolded |
| `vscode-extension` | Visual Studio Code | Node extension + `locr` (WASM) | wired |

## The golden rule

Every integration uses the **same core** (`locr.wasm`) and exposes the **same 3-line API**:

```js
import { imageToText } from 'locr';
const text = await imageToText(imageBytes);
```

No cloud keys, no subscription, no telemetry.

## Next steps

1. Office add-in: serve `packages/locr-js/dist/browser.js` and `locr_bg.wasm` next to `taskpane.html`, load `taskpane.js` as a module, and call `imageToText(file)` instead of the placeholder.
2. Google add-on: the HTML service cannot fetch package files, so inline the browser build and pass the `.wasm` bytes to it (mind Apps Script size limits: build without bundled models and call `loadModels()` with cached models).
3. Switch the VS Code extension's `file:` dependency to the published `locr` version.
4. Validate each manifest against its respective store (Microsoft AppSource, Google Workspace Marketplace, VS Code Marketplace).
