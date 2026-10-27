import io
import sys
import threading
import types

import pytest

import locr

native = pytest.mark.skipif(locr.image_to_data_json is None, reason="native extension not built")


def _png(img, **save):
    buf = io.BytesIO()
    img.save(buf, "PNG", **save)
    return buf.getvalue()


@native
def test_schema(page_png):
    data = locr.image_to_data(page_png)
    assert data["version"] == 1
    assert (data["width"], data["height"]) == (600, 360)
    assert data["transform"] == "none" and data["attempts"] == 1
    assert 0.0 <= data["score"] <= 1.0
    assert data["lines"], data
    assert data["text"] == "\n".join(line["text"] for line in data["lines"])
    for line in data["lines"]:
        assert set(line) == {"text", "confidence", "bbox", "words"}
        assert set(line["bbox"]) == {"x", "y", "width", "height"}
        for word in line["words"]:
            assert 0.0 < word["confidence"] <= 1.0
            assert word["bbox"]["x"] >= line["bbox"]["x"]


@native
def test_every_input_type_reads_the_same(page, page_png, tmp_path):
    expected = locr.image_to_data(page_png)
    path = tmp_path / "page.png"
    path.write_bytes(page_png)

    inputs = {
        "str": str(path),
        "Path": path,
        "bytearray": bytearray(page_png),
        "memoryview": memoryview(page_png),
        "file": open(path, "rb"),
        "PIL RGB": page,
        "PIL RGBA": page.convert("RGBA"),
        "PIL P": page.quantize(colors=256),  # lossless here: few colors
    }
    try:
        import numpy as np

        inputs["numpy RGB"] = np.asarray(page)
        inputs["numpy RGBA"] = np.asarray(page.convert("RGBA"))
    except ImportError:
        pass
    try:
        for name, image in inputs.items():
            assert locr.image_to_data(image) == expected, name
    finally:
        inputs["file"].close()

    assert locr.image_to_text(page_png) == expected["text"]
    assert locr.image_to_text(page) == expected["text"]


@native
def test_greyscale_inputs(page):
    grey = page.convert("L")
    expected = locr.image_to_data(_png(grey))
    assert locr.image_to_data(grey) == expected
    np = pytest.importorskip("numpy")
    assert locr.image_to_data(np.asarray(grey)) == expected
    assert locr.image_to_data(np.asarray(grey)[:, :, None]) == expected


@native
def test_pil_exif_orientation_is_applied(page, page_png):
    from PIL import Image

    exif = Image.Exif()
    exif[0x0112] = 6  # rotate 90 degrees clockwise to display
    rotated = _png(page.transpose(Image.Transpose.ROTATE_90), exif=exif)
    expected = locr.image_to_data(page_png)
    assert locr.image_to_data(rotated) == expected  # native decoder
    assert locr.image_to_data(Image.open(io.BytesIO(rotated))) == expected  # PIL path


@native
def test_options(page_png):
    digits = locr.image_to_data(page_png, allowed_chars="0123456789")
    assert all(c.isdigit() or c.isspace() for c in digits["text"]), digits["text"]

    assert locr.image_to_data(page_png, beam_width=4)["lines"]

    auto = locr.image_to_text_auto(page_png, min_score=1.5, max_attempts=3)
    assert auto["attempts"] == 3
    assert auto["score"] >= locr.image_to_text_scored(page_png)["score"]

    with pytest.raises(ValueError):
        locr.image_to_data(page_png, beam_width="wide")


@native
def test_errors(page):
    with pytest.raises(locr.LocrError, match="decode"):
        locr.image_to_text(b"definitely not an image")
    assert issubclass(locr.LocrError, RuntimeError)
    with pytest.raises(TypeError):
        locr.image_to_text(12345)
    np = pytest.importorskip("numpy")
    with pytest.raises(TypeError, match="uint8"):
        locr.image_to_text(np.zeros((4, 4), dtype=np.float32))
    with pytest.raises(ValueError, match="shape"):
        locr.image_to_text(np.zeros((4, 4, 2), dtype=np.uint8))


@native
def test_threads_share_the_engine(page_png):
    expected = locr.image_to_data(page_png)
    results = []
    threads = [threading.Thread(target=lambda: results.append(locr.image_to_data(page_png))) for _ in range(4)]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    assert results == [expected] * 4


def test_tesseract_fallback_schema(monkeypatch, page_png):
    calls = {}

    def image_to_data(img, config="", output_type=None):
        calls["config"] = config
        calls["size"] = img.size
        return {
            "level": [1, 4, 5, 5, 5, 5],
            "page_num": [1, 1, 1, 1, 1, 1],
            "block_num": [0, 1, 1, 1, 1, 1],
            "par_num": [0, 1, 1, 1, 1, 1],
            "line_num": [0, 1, 1, 1, 2, 2],
            "word_num": [0, 0, 1, 2, 1, 2],
            "left": [0, 20, 20, 130, 20, 80],
            "top": [0, 30, 30, 32, 80, 80],
            "width": [600, 200, 90, 90, 50, 10],
            "height": [360, 40, 40, 38, 20, 20],
            "conf": [-1, -1, 96.5, 90, -1, 95],
            "text": ["", "", "HELLO", "LOCR", "bye", " "],
        }

    fake = types.SimpleNamespace(image_to_data=image_to_data, Output=types.SimpleNamespace(DICT="dict"))
    monkeypatch.setitem(sys.modules, "pytesseract", fake)
    monkeypatch.setattr(locr, "image_to_data_json", None)
    monkeypatch.setattr(locr, "pixels_to_data_json", None)
    monkeypatch.setattr(locr, "image_to_text_bytes", None)

    data = locr.image_to_data(page_png, allowed_chars="HELOCR")
    assert calls == {"config": "-c tessedit_char_whitelist=HELOCR", "size": (600, 360)}
    assert data["text"] == "HELLO LOCR\nbye"
    assert (data["width"], data["height"], data["attempts"]) == (600, 360, 1)
    first, second = data["lines"]
    assert first["bbox"] == {"x": 20, "y": 30, "width": 200, "height": 40}
    assert [w["confidence"] for w in first["words"]] == [0.965, 0.9]
    assert first["confidence"] == pytest.approx((0.965 * 5 + 0.9 * 4) / 9)
    assert second["confidence"] is None
    assert data["confidence"] == pytest.approx(first["confidence"])
    assert 0.0 < data["score"] <= 1.0
    assert locr.image_to_text(page_png) == "HELLO LOCR\nbye"


def test_fallback_without_pytesseract(monkeypatch, page_png):
    monkeypatch.setitem(sys.modules, "pytesseract", None)
    monkeypatch.setattr(locr, "image_to_data_json", None)
    monkeypatch.setattr(locr, "image_to_text_bytes", None)
    with pytest.raises(ImportError, match="fallback"):
        locr.image_to_text(page_png)


def test_score_mirror():
    assert locr._score_text("") == 0.0
    assert locr._score_text("HELLO LOCR") > 0.6
    assert locr._score_text("@@@###~~~^^^") < 0.45
    assert locr._quality_score("HELLO LOCR", None) == locr._score_text("HELLO LOCR")
    assert locr._quality_score("HELLO LOCR", 0.99) > locr._quality_score("HELLO LOCR", 0.2)
