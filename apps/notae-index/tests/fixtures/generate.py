"""Regenerates the PDF fixtures stored beside this script.

Run with FIXTURE_DIR set to this directory:

    FIXTURE_DIR=apps/notae-index/tests/fixtures \
        python3 apps/notae-index/tests/fixtures/generate.py

import os
import struct
import zlib

OUT = os.environ["FIXTURE_DIR"]


def build(objects, root_ref, extra_trailer=b""):
    """Assemble a PDF from a list of object bodies (1-based index)."""
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
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


def text_page_mediabox(box):
    content = b"BT /F1 18 Tf 24 40 Td (Notae fixture) Tj ET"
    return content, box


# --- text-layer.pdf: one page, 200x100 pt, a real text layer ---------------
content = b"BT /F1 12 Tf 24 40 Td (Notae searchable text layer) Tj ET"
objects = [
    b"<< /Type /Catalog /Pages 2 0 R >>",
    b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100]"
    b" /Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
    stream(b"", content),
    b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
]
open(os.path.join(OUT, "text-layer.pdf"), "wb").write(build(objects, 1))

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
    b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100]"
    b" /Resources << /XObject << /Im0 5 0 R >> >> /Contents 4 0 R >>",
    stream(b"", b"q 200 0 0 100 0 0 cm /Im0 Do Q"),
    image,
]
open(os.path.join(OUT, "image-only.pdf"), "wb").write(build(objects, 1))

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
open(os.path.join(OUT, "multi-page.pdf"), "wb").write(build(objects, 1))

# --- corrupt.pdf: a plausible header followed by nothing usable ----------
corrupt = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
corrupt += b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n"
corrupt += b"trailer\n<< /Root 1 0 R >>\nstartxref\n999999\n%%EOF\n"
open(os.path.join(OUT, "corrupt.pdf"), "wb").write(bytes(corrupt))

# --- not-a-pdf.pdf: plain text wearing a .pdf name ----------------------
open(os.path.join(OUT, "not-a-pdf.pdf"), "w").write(
    "This file is plain text, not a PDF, but its name ends in .pdf.\n"
)

for name in sorted(os.listdir(OUT)):
    print(name, os.path.getsize(os.path.join(OUT, name)))