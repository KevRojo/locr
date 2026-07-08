# locr Google Workspace Add-on

Google Docs/Sheets/Slides add-on that runs `locr` OCR locally in the HTML service sidebar using an embedded WebAssembly module.

## Development

This project is a Google Apps Script project. Use [`clasp`](https://github.com/google/clasp) to push and deploy.

```bash
clasp login
clasp push
clasp open
```

## How it works

1. User opens the add-on from the **Add-ons → locr → Open sidebar** menu.
2. The sidebar loads the embedded `locr.wasm` (base64) and runs it entirely in the browser sandbox.
3. The user selects an image; the WASM returns the text.
4. The client calls `google.script.run.insertText(...)` to write the text into the document.

## TODO

- [ ] Replace `WASM_PLACEHOLDER` in `sidebar.js` with the base64 output of `locr.wasm`.
- [ ] Add Sheet/Slides-specific `insertText` server functions.
- [ ] Publish to Google Workspace Marketplace after the engine is real.
