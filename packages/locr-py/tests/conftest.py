import io
import os
from pathlib import Path

import pytest

REPO = Path(__file__).resolve().parents[3]
FIXTURES = REPO / "crates" / "locr-core" / "tests" / "fixtures"

# Wheels built without bundled models (`maturin develop --no-default-features`)
# load the tiny synthetic test models. Bundled builds ignore this.
os.environ.setdefault("LOCR_MODELS_DIR", str(FIXTURES / "synthetic"))


@pytest.fixture(scope="session")
def page():
    """RGB page with real text (hello.png) and dark-grey glyph rows that the
    synthetic test models detect, so tests hold with either model set."""
    from PIL import Image

    img = Image.new("RGB", (600, 360), "white")
    img.paste(Image.open(FIXTURES / "hello.png").convert("RGB"), (0, 0))
    for row in range(3):
        y = 150 + row * 70
        for i in range(4 + row):
            grey = 30 + (row * 31 + i * 17) % 90
            x = 20 + i * 16 + (i // 3) * 30
            img.paste((grey, grey, grey), (x, y, x + 12, y + 22))
    return img


@pytest.fixture(scope="session")
def page_png(page):
    buf = io.BytesIO()
    page.save(buf, "PNG")
    return buf.getvalue()
