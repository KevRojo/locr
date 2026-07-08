# locr-py

Local OCR for Python. Part of the [locr](https://github.com/KevRojo/locr) project.

## Install

```bash
pip install locr
```

The published wheel ships a pure-Rust OCR engine (ocrs + RTen) with models
embedded at build time. No system Tesseract required.

Optional fallback if a native wheel is not available for your platform:

```bash
pip install 'locr[fallback]'
```

That path needs a system [Tesseract](https://tesseract-ocr.github.io/) binary.

## Quickstart

```python
from locr import image_to_text, image_to_text_scored, image_to_text_auto

text = image_to_text("invoice.png")
print(text)

# Quality score (0..1) — single pass
r = image_to_text_scored("invoice.png")
print(r["text"], r["score"])

# Superpower: if score is low, auto-try contrast/brightness/saturation/etc.
r = image_to_text_auto("dark_scan.png", min_score=0.55)
print(r["text"], r["score"], r["transform"], r["attempts"])

# Also accepts raw bytes
with open("invoice.png", "rb") as f:
    text = image_to_text(f.read())
```

## License

MIT (code). Bundled models: CC-BY-SA 4.0 — see the monorepo `NOTICE-MODELS`.
