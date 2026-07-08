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
from locr import image_to_text

text = image_to_text("invoice.png")
print(text)

# Also accepts raw bytes
with open("invoice.png", "rb") as f:
    text = image_to_text(f.read())
```

## License

MIT (code). Bundled models: CC-BY-SA 4.0 — see the monorepo `NOTICE-MODELS`.
