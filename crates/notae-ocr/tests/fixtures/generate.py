#!/usr/bin/env python3
"""Generates deterministic grayscale text images used as OCR fixtures.

The images are written next to this script as PNG files. They are committed to
the repository so the test suite does not depend on Python, PIL, or a font
file being installed at run time.

The text was chosen to exercise both English and Polish recognition:

* ``english.png`` contains ``Notae searchable`` in DejaVu Sans.
* ``polish.png`` contains ``Zażółć gęślą jaźń`` — the canonical Polish
  pangram that covers every diacritic in the Polish alphabet.

Run this script once after touching the wording or the font: the tests assume
the PNG files exist and the recognised text matches.
"""

from __future__ import annotations

import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

FONT_PATH = Path("/tmp/DejaVuSans.ttf")
FONT_BOLD_PATH = Path("/tmp/DejaVuSans-Bold.ttf")
PADDING = 24


def render(text: str, target: Path, font_size: int = 36) -> None:
    if not FONT_PATH.exists() or not FONT_BOLD_PATH.exists():
        sys.exit(
            "DejaVuSans.ttf and DejaVuSans-Bold.ttf are required at "
            f"{FONT_PATH} and {FONT_BOLD_PATH}; see crates/notae-ocr/README.md."
        )

    font = ImageFont.truetype(str(FONT_BOLD_PATH), font_size)
    # Measure first so the canvas is just big enough. ``getbbox`` includes the
    # ascenders and descenders; ``getlength`` is the advance width of the text.
    left, top, right, bottom = font.getbbox(text)
    width = int(right - left) + PADDING * 2
    height = int(bottom - top) + PADDING * 2

    image = Image.new("L", (width, height), 255)
    draw = ImageDraw.Draw(image)
    # PIL draws text from the top-left anchor; shift by the font's left bearing
    # so the glyphs sit flush against the padding.
    draw.text((PADDING - left, PADDING - top), text, fill=0, font=font)

    image.save(target, format="PNG")
    print(f"wrote {target} ({width}x{height})")


def main() -> None:
    here = Path(__file__).resolve().parent
    render("Notae searchable", here / "english.png")
    render("Zażółć gęślą jaźń", here / "polish.png")


if __name__ == "__main__":
    main()