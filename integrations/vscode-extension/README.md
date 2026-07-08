# locr VS Code Extension

VS Code extension that runs `locr` OCR locally on images from the workspace.

## Development

```bash
npm install
npm run compile
```

Then press F5 in VS Code to run the extension in a new Extension Development Host window.

## Commands

- `locr: OCR de imagen` — pick an image file and insert the extracted text into the active editor.

## How it works

The extension loads the `locr` npm package (or a prebuilt native N-API binary) and calls `imageToText(imageBytes)` on the selected image. Everything stays on the user's machine.

## TODO

- [ ] Replace placeholder OCR with real `locr` import once the engine is wired.
- [ ] Add support for right-clicking an image file in the Explorer.
- [ ] Add support for OCR from the clipboard.
