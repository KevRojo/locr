"""locr - local OCR for Python. No cloud APIs, no paywalls."""
from __future__ import annotations

import io
from pathlib import Path
from typing import Any, Dict, Optional, Union

try:
    from ._locr import (  # Rust extension (pure local OCR)
        image_to_text_auto_bytes,
        image_to_text_bytes,
        image_to_text_scored_bytes,
    )
except ImportError:
    image_to_text_bytes = None  # type: ignore[assignment]
    image_to_text_scored_bytes = None  # type: ignore[assignment]
    image_to_text_auto_bytes = None  # type: ignore[assignment]


__version__ = "0.1.0"
__all__ = ["image_to_text", "image_to_text_scored", "image_to_text_auto"]


PathLike = Union[str, Path]


def _read_path(image: PathLike) -> bytes:
    with open(image, "rb") as f:
        return f.read()


def _as_bytes(image: Union[PathLike, bytes]) -> bytes:
    if isinstance(image, (str, Path)):
        return _read_path(image)
    return image


def _open_image(image: Union[PathLike, bytes]):
    """Open an image for the optional Tesseract fallback."""
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


def _score_text(text: str) -> float:
    """Pure-Python mirror of the Rust heuristic (fallback path only)."""
    text = text.strip()
    if not text:
        return 0.0
    n = len(text)
    length_score = min(n / 12.0, 1.0)
    good = sum(
        1
        for c in text
        if c.isalnum() or c.isspace() or c in ".,:-/%$#"
    )
    alnum_score = good / n
    words = [w for w in text.split() if len(w) >= 2]
    word_score = {0: 0.1, 1: 0.55, 2: 0.8}.get(len(words), 1.0) if words else 0.1
    if len(words) >= 3:
        word_score = 1.0
    weird = sum(
        1
        for c in text
        if not (c.isalnum() or c.isspace() or c in ".,:;-/%$#()[]\"'!?")
    )
    noise = min(weird / n, 0.6)
    raw = 0.25 * length_score + 0.35 * alnum_score + 0.30 * word_score + 0.10 * (1.0 - noise)
    return max(0.0, min(1.0, raw - noise * 0.4))


def image_to_text(image: Union[PathLike, bytes]) -> str:
    """Extract text from an image locally.

    Prefers the pure-Rust extension (ocrs/RTen, models embedded at build time).
    If the extension is unavailable, falls back to the optional Tesseract path
    via ``pip install 'locr[fallback]'``.
    """
    if image_to_text_bytes is not None:
        return image_to_text_bytes(_as_bytes(image))
    return _tesseract_fallback(image)


def image_to_text_scored(image: Union[PathLike, bytes]) -> Dict[str, Any]:
    """OCR with quality score (single pass).

    Returns:
        dict with keys: text, score (0..1), transform, attempts
    """
    if image_to_text_scored_bytes is not None:
        return image_to_text_scored_bytes(_as_bytes(image))

    text = _tesseract_fallback(image)
    return {
        "text": text,
        "score": _score_text(text),
        "transform": "none",
        "attempts": 1,
    }


def image_to_text_auto(
    image: Union[PathLike, bytes],
    min_score: float = 0.55,
    max_attempts: int = 7,
) -> Dict[str, Any]:
    """Superpower: OCR + auto-enhance when the score is low.

    If the first pass scores below ``min_score``, the engine retries with
    contrast / brightness / saturation / grayscale / invert transforms and
    keeps the best scoring result.

    Returns:
        dict with keys: text, score (0..1), transform, attempts
    """
    if image_to_text_auto_bytes is not None:
        return image_to_text_auto_bytes(_as_bytes(image), min_score, max_attempts)

    # Fallback path: single tesseract pass (no enhance loop without Rust).
    text = _tesseract_fallback(image)
    return {
        "text": text,
        "score": _score_text(text),
        "transform": "none",
        "attempts": 1,
    }
