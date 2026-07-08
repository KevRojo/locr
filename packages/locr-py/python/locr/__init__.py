"""locr - local OCR for Python. No cloud APIs, no paywalls."""
from __future__ import annotations

import io
from pathlib import Path
from typing import Union

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


def _open_image(image: Union[PathLike, bytes]):
    """Open an image for the optional Tesseract fallback.

    Accepts a filesystem path or raw image bytes (wrapped in BytesIO).
    """
    from PIL import Image  # type: ignore

    if isinstance(image, (str, Path)):
        return Image.open(str(image))
    return Image.open(io.BytesIO(image))


def _tesseract_fallback(image: Union[PathLike, bytes]) -> str:
    """Optional fallback when the Rust extension is not built.

    Requires the optional extras: ``pip install locr[fallback]``
    and a system Tesseract binary.
    """
    try:
        import pytesseract  # type: ignore
    except ImportError as e:
        raise ImportError(
            "locr Rust extension is not available and the optional Tesseract "
            "fallback is not installed. Either install a wheel with the native "
            "extension, or run: pip install 'locr[fallback]'"
        ) from e

    img = _open_image(image)
    return pytesseract.image_to_string(img).strip()


def image_to_text(image: Union[PathLike, bytes]) -> str:
    """Extract text from an image locally.

    Prefers the pure-Rust extension (ocrs/RTen, models embedded at build time).
    If the extension is unavailable, falls back to the optional Tesseract path
    via ``pip install 'locr[fallback]'``.

    Args:
        image: Path to an image file, or raw image bytes.

    Returns:
        The recognized text as a string.
    """
    if image_to_text_bytes is not None:
        data = _read_path(image) if isinstance(image, (str, Path)) else image
        return image_to_text_bytes(data)

    return _tesseract_fallback(image)
