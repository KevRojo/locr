# locr-cli

`locr` on the command line: images in, text or JSON out. Fully local — the OCR
models are embedded in the binary, nothing is uploaded anywhere.

```bash
cargo install locr-cli

locr invoice.png                         # text
locr --json scan-*.jpg > results.jsonl   # lines, words, boxes, confidence
locr --auto --allowed-chars 0123456789 meter.jpg
curl -s https://example.com/sign.png | locr -
```

`--json` prints one object per image (JSON Lines) in the locr result schema,
plus a `source` field. Exit codes: `0` success, `1` an image failed (the
others are still processed), `2` usage error.

Run `locr --help` for every option.
