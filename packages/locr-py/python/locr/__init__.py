"""locr - local OCR for Python."""
from __future__ import annotations

from pathlib import Path
from typing import Union

from PIL import Image
import pytesseract

try:
    from ._locr import image_to_text_bytes  # Rust extension (optional)
except ImportError:
    image_to_text_bytes = None  # type: ignore[assignment]


__version__ = "0.1.0"
__all__ = ["image_to_text"]


PathLike = Union[str, Path]


def image_to_text(image: Union[PathLike, bytes]) -> str:
    """Extract text from an image locally.

    Args:
        image: Path to an image file, or raw image bytes.

    Returns:
        The recognized text as a string.
    """
    if isinstance(image, (str, Path)):
        if image_to_text_bytes is not None:
            with open(image, "rb") as f:
                return image_to_text_bytes(f.read())
        return pytesseract.image_to_string(str(image)).strip()

    if image_to_text_bytes is not None:
        return image_to_text_bytes(image)

    img = Image.open(image)
    return pytesseract.image_to_string(img).strip()
