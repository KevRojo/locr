# locr VS Code Extension

VS Code extension that runs `locr` OCR locally on images from the workspace.

## Development

```bash
# The extension depends on the local npm package until `locr` is published.
(cd ../../packages/locr-js && npm install && npm run build)
npm install
npm run compile
```

Then press F5 in VS Code to run the extension in a new Extension Development Host window.

## Commands

- `locr: OCR de imagen` — pick an image file and insert the extracted text into the active editor.

## How it works

The extension calls `imageToText(imageBytes)` from the `locr` npm package, which runs the Rust OCR engine as WebAssembly inside the extension host. Everything stays on the user's machine.

## TODO

- [x] Real OCR through the `locr` npm package.
- [x] Right-click an image file in the Explorer.
- [ ] Add support for OCR from the clipboard.
