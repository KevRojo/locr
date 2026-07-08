"""locr - local OCR for Python. No cloud APIs, no paywalls."""
from __future__ import annotations

from pathlib import Path
from typing import Union

from PIL import Image
import pytesseract

try:
    from ._locr import image_to_text_bytes  # Rust extension (pure local OCR)
except ImportError:
    image_to_text_bytes = None  # type: ignore[assignment]


__version__ = "0.1.0"
__all__ = ["image_to_text"]


PathLike = Union[str, Path]


def _read_path(image: PathLike) -> bytes:
    with open(image, "rb") as f:
        return f.read()


def image_to_text(image: Union[PathLike, bytes]) -> str:
    """Extract text from an image locally.

    The Rust extension processes the image on-device using a pure-Rust OCR
    engine. If the extension is unavailable, it falls back to the local
    Tesseract CLI via pytesseract.

    Args:
        image: Path to an image file, or raw image bytes.

    Returns:
        The recognized text as a string.
    """
    if isinstance(image, (str, Path)):
        if image_to_text_bytes is not None:
            return image_to_text_bytes(_read_path(image))
        return pytesseract.image_to_string(str(image)).strip()

    if image_to_text_bytes is not None:
        return image_to_text_bytes(image)

    img = Image.open(image)
    return pytesseract.image_to_string(img).strip()
