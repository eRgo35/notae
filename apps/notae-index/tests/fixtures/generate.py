"""Regenerates the PDF fixtures stored beside this script.

Run with FIXTURE_DIR set to this directory:

    FIXTURE_DIR=apps/notae-index/tests/fixtures \
        python3 apps/notae-index/tests/fixtures/generate.py

"""

import os
import zlib

OUT = os.environ["FIXTURE_DIR"]


HEADER = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n"


def build(objects, root_ref, extra_trailer=b"", header=HEADER):
    """Assemble a PDF from a list of object bodies (1-based index).

    `header` defaults to a normal `%PDF-` header; passing something else
    builds a structurally complete file that is missing, or displaced
    behind, the mandatory header.
    """
    out = bytearray(header)
    offsets = []
    for index, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += b"%d 0 obj\n" % index
        out += body
        out += b"\nendobj\n"
    xref_at = len(out)
    count = len(objects) + 1
    out += b"xref\n0 %d\n" % count
    out += b"0000000000 65535 f \n"
    for offset in offsets:
        out += b"%010d 00000 n \n" % offset
    out += b"trailer\n<< /Size %d /Root %d 0 R%s >>\n" % (count, root_ref, extra_trailer)
    out += b"startxref\n%d\n%%%%EOF\n" % xref_at
    return bytes(out)


def stream(dict_body, data, compress=False):
    if compress:
        data = zlib.compress(data)
        dict_body = dict_body + b" /Filter /FlateDecode"
    return b"<< " + dict_body + b" /Length %d >>\nstream\n" % len(data) + data + b"\nendstream"


def write_fixture(name, data, mode="wb"):
    with open(os.path.join(OUT, name), mode) as handle:
        handle.write(data)


def text_page_mediabox(box):
    content = b"BT /F1 18 Tf 24 40 Td (Notae fixture) Tj ET"
    return content, box


# --- text-layer.pdf: one page, 200x100 pt, a real text layer ---------------
content = b"BT /F1 12 Tf 24 40 Td (Notae searchable text layer) Tj ET"
objects = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    (b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100]"
     b" /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"),
    stream(b"", content),
    b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
]
write_fixture("text-layer.pdf", build(objects, 1))

# --- image-only.pdf: one page carrying a JPEG, no text operators ----------
# A 64x32 RGB gradient, stored as a Flate-compressed raw image XObject.
width, height = 64, 32
pixels = bytearray()
for y in range(height):
    for x in range(width):
        pixels += bytes([x * 4 % 256, y * 8 % 256, 128])
image = stream(
    b"/Type /XObject /Subtype /Image /Width %d /Height %d"
    b" /ColorSpace /DeviceRGB /BitsPerComponent 8" % (width, height),
    bytes(pixels),
    compress=True,
)
objects = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    (b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100]"
     b" /Resources << /XObject << /Im0 5 0 R >> >> /Contents 4 0 R >>"),
    stream(b"", b"q 200 0 0 100 0 0 cm /Im0 Do Q"),
    image,
]
write_fixture("image-only.pdf", build(objects, 1))

# --- multi-page.pdf: three pages, each with a text layer -----------------
kids = b"[3 0 R 5 0 R 7 0 R]"
objects = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids " + kids + b" /Count 3 >>",
]
for number, letter in enumerate(b"ABC", start=1):
    objects.append(
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100]"
        b" /Resources << /Font << /F1 9 0 R >> >> /Contents %d 0 R >>" % (2 + 2 * number)
    )
    objects.append(
        stream(b"", b"BT /F1 12 Tf 24 40 Td (Notae page %d) Tj ET" % number)
    )
objects.append(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>")
write_fixture("multi-page.pdf", build(objects, 1))

# --- corrupt.pdf: a plausible header followed by nothing usable ----------
corrupt = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
corrupt += b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n"
corrupt += b"trailer\n<< /Root 1 0 R >>\nstartxref\n999999\n%%EOF\n"
write_fixture("corrupt.pdf", bytes(corrupt))

# --- not-a-pdf.pdf: plain text wearing a .pdf name ----------------------
write_fixture(
    "not-a-pdf.pdf",
    "This file is plain text, not a PDF, but its name ends in .pdf.\n",
    "w",
)

# --- damaged-contents.pdf: a valid page tree whose /Contents is corrupt --
# The page itself is well formed and its /Contents entry is present, but the
# stream claims an ASCIIHexDecode filter over bytes that are not hex at all,
# so decoding it fails. This is the shape of a file truncated or rewritten in
# transit: the structure survives, the page payload does not.
damaged = stream(b"/Filter /ASCIIHexDecode", b"this is not hexadecimal ZZZ")
objects = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    (b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100]"
     b" /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"),
    damaged,
    b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
]
write_fixture("damaged-contents.pdf", build(objects, 1))

# --- hex-contents.pdf: the same unusual-but-valid filter, intact --------
# ASCIIHexDecode is a perfectly ordinary PDF filter. This page proves that
# rejecting damaged streams must not mean rejecting uncommon filters.
hexed = content = b"BT /F1 12 Tf 24 40 Td (Notae hex filter) Tj ET"
hexed = stream(b"/Filter /ASCIIHexDecode", content.hex().encode())
objects = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    (b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100]"
     b" /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"),
    hexed,
    b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
]
write_fixture("hex-contents.pdf", build(objects, 1))

# --- no-header.pdf: PDF-shaped objects with no %PDF- header at all ------
# The cross-reference table, the objects and the trailer are all valid; only
# the mandatory header is absent. A parser that reconstructs structure by
# scanning will happily accept this, which is the point.
objects = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    (b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100]"
     b" /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"),
    stream(b"", b"BT /F1 12 Tf 24 40 Td (Notae no header) Tj ET"),
    b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
]
write_fixture(
    "no-header.pdf",
    build(
        objects,
        1,
        header=b"NOT-A-PDF-HEADER, just some bytes that happen to parse.\n",
    ),
)

# --- leading-junk.pdf: the same document behind 32 bytes of junk -------
# Real files arrive this way: an HTTP error page, a mail preamble, or an
# archive stub prepended to the document. The header is not at byte zero,
# but it is inside the first 1024 bytes, which is where the specification
# says a reader must look for it.
objects = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    (b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100]"
     b" /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"),
    stream(b"", b"BT /F1 12 Tf 24 40 Td (Notae leading junk) Tj ET"),
    b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
]
write_fixture(
    "leading-junk.pdf",
    build(objects, 1, header=b"junk from an archive stub!!!!\n" + HEADER),
)

# --- word-spacing.pdf: word boundaries carried by TJ, not by spaces -----
# `[(Notae) -250 (searchable)] TJ` is how a typesetter says "these two
# strings are two words": the number shifts the second string along the
# baseline by a quarter of an em, and no space character is ever drawn.
objects = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    (b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100]"
     b" /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"),
    stream(
        b"",
        b"BT /F1 12 Tf 24 40 Td [(Notae) -250 (searchable)] TJ ET",
    ),
    b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
]
write_fixture("word-spacing.pdf", build(objects, 1))

for name in sorted(os.listdir(OUT)):
    print(name, os.path.getsize(os.path.join(OUT, name)))