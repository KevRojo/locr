# locr Microsoft Office Add-in

Office Add-in (Word, Excel, PowerPoint, Outlook) that runs `locr` OCR locally in the task pane via WebAssembly.

## Development

```bash
npm install
npm run dev
```

Then sideload `manifest.localhost.xml` into Word/Excel/PowerPoint/Outlook.

## How it works

1. User selects an image in the document (or pastes an image into the task pane).
2. The task pane loads the `locr.wasm` module and runs OCR on the image bytes.
3. The extracted text is inserted into the active document or copied to the clipboard.

## TODO

- [ ] Replace `taskpane.js` placeholder OCR with a real `locr.wasm` call.
- [ ] Build and copy `../../crates/locr-wasm/pkg` into this folder.
- [ ] Add file-drop support in the task pane.
- [ ] Validate the manifest with `npm run validate`.
