# locr-py

Local OCR for Python. Part of the [locr](https://github.com/KevRojo/locr) project.

## Install

```bash
pip install locr
```

The published wheel ships a pure-Rust OCR engine (ocrs + RTen) with models
embedded at build time. No system Tesseract required. Wheels use the stable
ABI (`abi3`): one wheel per platform covers CPython 3.9 and newer.

Optional fallback if a native wheel is not available for your platform:

```bash
pip install 'locr[fallback]'
```

That path needs a system [Tesseract](https://tesseract-ocr.github.io/) binary.

## Quickstart

```python
from locr import image_to_text, image_to_data, image_to_text_auto

text = image_to_text("invoice.png")
print(text)

# Lines, words, bounding boxes and confidence (0..1)
data = image_to_data("invoice.png")
for line in data["lines"]:
    print(line["text"], line["bbox"], line["confidence"])
    for word in line["words"]:
        print("  ", word["text"], word["bbox"], word["confidence"])

# Only digits (meter readings, totals, IDs). Spaces are always kept.
total = image_to_data("receipt.jpg", allowed_chars="0123456789.,")["text"]

# Superpower: if the score is low, auto-try contrast/brightness/saturation/etc.
r = image_to_text_auto("dark_scan.png", min_score=0.55)
print(r["text"], r["score"], r["transform"], r["attempts"])
```

`image_to_data` options: `allowed_chars`, `beam_width` (CTC beam search;
`0`/`1` = greedy), `auto_enhance`, `min_score`, `max_attempts`.

Every function accepts:

- a path (`str` / `pathlib.Path`)
- encoded bytes: `bytes`, `bytearray`, `memoryview`
- a binary file object (`open(..., "rb")`, `io.BytesIO`, uploads)
- a PIL image (EXIF orientation is applied, like for files)
- a `uint8` numpy array shaped `(H, W)` or `(H, W, 1|3|4)` in RGB(A) order —
  for OpenCV's BGR arrays use `cv2.cvtColor(img, cv2.COLOR_BGR2RGB)` first

Photos are read upright (EXIF orientation), and transparent PNGs are flattened
onto a contrasting background before OCR.

OCR runs with the GIL released, so thread pools (and web servers) process
images in parallel. Failures raise `locr.LocrError` (a `RuntimeError`);
invalid arguments raise `ValueError` / `TypeError`.

## Development

```bash
pip install maturin pytest pillow numpy
maturin develop --release              # bundles the models (downloads them once)
pytest tests

# Offline: skip the real models; tests then use tiny synthetic ones.
maturin develop --no-default-features
pytest tests
```

## License

MIT (code). Bundled models: CC-BY-SA 4.0 — see the monorepo `NOTICE-MODELS`.
