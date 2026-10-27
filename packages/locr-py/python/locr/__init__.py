"""locr - local OCR for Python. No cloud APIs, no paywalls."""
from __future__ import annotations

import io
import json
import os
from typing import Any, Dict, List, Optional, Tuple, Union

try:
    from ._locr import (  # Rust extension (pure local OCR)
        LocrError,
        image_to_data_json,
        image_to_text_bytes,
        pixels_to_data_json,
    )
except ImportError:
    image_to_text_bytes = None  # type: ignore[assignment]
    image_to_data_json = None  # type: ignore[assignment]
    pixels_to_data_json = None  # type: ignore[assignment]

    class LocrError(RuntimeError):  # type: ignore[no-redef]
        """OCR failed: the image could not be decoded, the models are missing, or the engine errored."""


__version__ = "0.1.0"
__all__ = [
    "LocrError",
    "image_to_data",
    "image_to_text",
    "image_to_text_auto",
    "image_to_text_scored",
]


# Paths, encoded bytes (bytes / bytearray / memoryview), binary file objects,
# PIL images, or uint8 numpy arrays shaped (H, W) or (H, W, 1|3|4) in RGB(A) order.
ImageInput = Any

# Normalized input: ("bytes", encoded) or ("pixels", (width, height, raw)).
_Loaded = Tuple[str, Any]


def _load(image: ImageInput) -> _Loaded:
    if isinstance(image, (str, os.PathLike)):
        with open(image, "rb") as f:
            return "bytes", f.read()
    if isinstance(image, bytes):
        return "bytes", image
    if isinstance(image, (bytearray, memoryview)):
        return "bytes", bytes(image)
    if hasattr(image, "read"):
        return "bytes", image.read()
    if hasattr(image, "mode") and hasattr(image, "size") and hasattr(image, "tobytes"):
        return "pixels", _pil_pixels(image)
    if hasattr(image, "shape") and hasattr(image, "dtype") and hasattr(image, "tobytes"):
        return "pixels", _array_pixels(image)
    raise TypeError(f"unsupported image type: {type(image).__name__}")


def _pil_pixels(image: Any) -> Tuple[int, int, bytes]:
    try:
        from PIL import ImageOps  # type: ignore

        # Match the path/bytes inputs, which honor EXIF orientation.
        image = ImageOps.exif_transpose(image)
    except ImportError:
        pass
    if image.mode not in ("L", "RGB", "RGBA"):
        has_alpha = image.mode in ("LA", "La", "PA", "RGBa") or "transparency" in image.info
        image = image.convert("RGBA" if has_alpha else "RGB")
    width, height = image.size
    return width, height, image.tobytes()


def _array_pixels(array: Any) -> Tuple[int, int, bytes]:
    if str(array.dtype) != "uint8":
        raise TypeError(f"numpy arrays must be uint8, got {array.dtype}")
    shape = tuple(array.shape)
    if len(shape) == 2 or (len(shape) == 3 and shape[2] in (1, 3, 4)):
        height, width = shape[0], shape[1]
        return width, height, array.tobytes()
    raise ValueError(f"expected an (H, W) or (H, W, 1|3|4) array, got shape {shape}")


def _options_json(
    allowed_chars: Optional[str],
    beam_width: int,
    auto_enhance: bool,
    min_score: float,
    max_attempts: int,
) -> str:
    options: Dict[str, Any] = {}
    if allowed_chars is not None:
        options["allowed_chars"] = str(allowed_chars)
    if beam_width:
        options["beam_width"] = int(beam_width)
    if auto_enhance:
        options.update(auto_enhance=True, min_score=float(min_score), max_attempts=int(max_attempts))
    return json.dumps(options)


def _data(loaded: _Loaded, options_json: str, allowed_chars: Optional[str]) -> Dict[str, Any]:
    kind, payload = loaded
    if image_to_data_json is not None:
        if kind == "bytes":
            raw = image_to_data_json(payload, options_json)
        else:
            width, height, pixels = payload
            raw = pixels_to_data_json(width, height, pixels, options_json)
        return json.loads(raw)
    return _tesseract_data(loaded, allowed_chars)


def image_to_text(image: ImageInput) -> str:
    """Extract text from an image locally.

    Accepts a path, encoded bytes (``bytes``/``bytearray``/``memoryview``),
    a binary file object, a PIL image, or a uint8 numpy array (RGB/RGBA).

    Prefers the pure-Rust extension (ocrs/RTen, models embedded at build time).
    If the extension is unavailable, falls back to the optional Tesseract path
    via ``pip install 'locr[fallback]'``.
    """
    loaded = _load(image)
    if loaded[0] == "bytes" and image_to_text_bytes is not None:
        return image_to_text_bytes(loaded[1])
    return _data(loaded, "{}", None)["text"]


def image_to_data(
    image: ImageInput,
    *,
    allowed_chars: Optional[str] = None,
    beam_width: int = 0,
    auto_enhance: bool = False,
    min_score: float = 0.55,
    max_attempts: int = 7,
) -> Dict[str, Any]:
    """Full OCR result: text, lines, words, bounding boxes and confidence.

    Args:
        image: see :func:`image_to_text`.
        allowed_chars: only emit these characters, e.g. ``"0123456789"``.
            Spaces are always allowed.
        beam_width: CTC beam search width; ``0``/``1`` = greedy (fastest).
        auto_enhance: retry with contrast/brightness/saturation/... transforms
            while the score is below ``min_score`` (up to ``max_attempts`` passes).

    Returns:
        dict with keys ``version``, ``text``, ``score`` (0..1), ``confidence``
        (0..1 or None), ``transform``, ``attempts``, ``width``, ``height`` and
        ``lines``: ``[{"text", "confidence", "bbox": {"x", "y", "width",
        "height"}, "words": [{"text", "confidence", "bbox"}]}]``. Boxes are
        pixels of the image as displayed (EXIF orientation applied).
    """
    options = _options_json(allowed_chars, beam_width, auto_enhance, min_score, max_attempts)
    return _data(_load(image), options, allowed_chars)


def image_to_text_scored(image: ImageInput) -> Dict[str, Any]:
    """OCR with quality score (single pass).

    Returns:
        the :func:`image_to_data` dict; ``text``, ``score`` (0..1),
        ``transform`` and ``attempts`` included.
    """
    return image_to_data(image)


def image_to_text_auto(
    image: ImageInput,
    min_score: float = 0.55,
    max_attempts: int = 7,
) -> Dict[str, Any]:
    """Superpower: OCR + auto-enhance when the score is low.

    If the first pass scores below ``min_score``, the engine retries with
    contrast / brightness / saturation / grayscale / invert transforms and
    keeps the best scoring result.

    Returns:
        the :func:`image_to_data` dict; ``text``, ``score`` (0..1),
        ``transform`` and ``attempts`` included.
    """
    return image_to_data(image, auto_enhance=True, min_score=min_score, max_attempts=max_attempts)


# --- Optional Tesseract fallback (no native extension) ----------------------


def _pil_image(loaded: _Loaded) -> Any:
    """Open an image for the optional Tesseract fallback."""
    from PIL import Image, ImageOps  # type: ignore

    kind, payload = loaded
    if kind == "bytes":
        return ImageOps.exif_transpose(Image.open(io.BytesIO(payload)))
    width, height, pixels = payload
    mode = {1: "L", 3: "RGB", 4: "RGBA"}[len(pixels) // (width * height)]
    return Image.frombytes(mode, (width, height), pixels)


def _tesseract_data(loaded: _Loaded, allowed_chars: Optional[str]) -> Dict[str, Any]:
    """Optional fallback when the Rust extension is not built.

    Requires the optional extras: ``pip install locr[fallback]``
    and a system Tesseract binary. Single pass, same schema as the extension.
    """
    try:
        import pytesseract  # type: ignore
    except ImportError as e:
        raise ImportError(
            "locr Rust extension is not available and the optional Tesseract "
            "fallback is not installed. Either install a wheel with the native "
            "extension, or run: pip install 'locr[fallback]'"
        ) from e

    img = _pil_image(loaded)
    config = f"-c tessedit_char_whitelist={allowed_chars}" if allowed_chars else ""
    data = pytesseract.image_to_data(img, config=config, output_type=pytesseract.Output.DICT)

    lines: List[Dict[str, Any]] = []
    keys: List[Tuple[Any, ...]] = []
    for i, raw_text in enumerate(data["text"]):
        text = str(raw_text).strip()
        if int(data["level"][i]) != 5 or not text:
            continue
        conf = float(data["conf"][i])
        word = {
            "text": text,
            "confidence": min(conf / 100.0, 1.0) if conf >= 0 else None,
            "bbox": {k: int(data[src][i]) for k, src in (("x", "left"), ("y", "top"), ("width", "width"), ("height", "height"))},
        }
        key = (data["page_num"][i], data["block_num"][i], data["par_num"][i], data["line_num"][i])
        if keys and keys[-1] == key:
            line = lines[-1]
            line["text"] += " " + text
            line["words"].append(word)
            line["bbox"] = _union(line["bbox"], word["bbox"])
        else:
            keys.append(key)
            lines.append({"text": text, "confidence": None, "bbox": dict(word["bbox"]), "words": [word]})

    for line in lines:
        line["confidence"] = _char_weighted(line["words"])
    text = "\n".join(line["text"] for line in lines)
    confidence = _char_weighted([w for line in lines for w in line["words"]])
    return {
        "version": 1,
        "text": text,
        "score": _quality_score(text, confidence),
        "confidence": confidence,
        "transform": "none",
        "attempts": 1,
        "width": img.width,
        "height": img.height,
        "lines": lines,
    }


def _union(a: Dict[str, int], b: Dict[str, int]) -> Dict[str, int]:
    x, y = min(a["x"], b["x"]), min(a["y"], b["y"])
    right = max(a["x"] + a["width"], b["x"] + b["width"])
    bottom = max(a["y"] + a["height"], b["y"] + b["height"])
    return {"x": x, "y": y, "width": right - x, "height": bottom - y}


def _char_weighted(words: List[Dict[str, Any]]) -> Optional[float]:
    """Mean word confidence weighted by length, like the native extension."""
    pairs = [(w["confidence"], len(w["text"])) for w in words if w["confidence"] is not None]
    total = sum(n for _, n in pairs)
    return sum(c * n for c, n in pairs) / total if total else None


def _quality_score(text: str, confidence: Optional[float]) -> float:
    """Pure-Python mirror of the Rust score: confidence blended with the heuristic."""
    heuristic = _score_text(text)
    if confidence is None or heuristic <= 0.0:
        return heuristic
    return max(0.0, min(1.0, 0.6 * confidence + 0.4 * heuristic))


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
    word_score = {0: 0.1, 1: 0.55, 2: 0.8}.get(len(words), 1.0)
    weird = sum(
        1
        for c in text
        if not (c.isalnum() or c.isspace() or c in ".,:;-/%$#()[]\"'!?")
    )
    noise = min(weird / n, 0.6)
    raw = 0.25 * length_score + 0.35 * alnum_score + 0.30 * word_score + 0.10 * (1.0 - noise)
    return max(0.0, min(1.0, raw - noise * 0.4))
