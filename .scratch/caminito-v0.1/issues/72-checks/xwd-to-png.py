#!/usr/bin/env python3
"""Turns an `xwd -root` dump of a TrueColor screen with 32 bits for each pixel into a PNG.

  xwd-to-png.py FILE.xwd FILE.png
"""
import struct
import sys

import gi

gi.require_version("GdkPixbuf", "2.0")
from gi.repository import GdkPixbuf, GLib  # noqa: E402

data = open(sys.argv[1], "rb").read()
header = struct.unpack(">25I", data[:100])
header_size, width, height = header[0], header[4], header[5]
byte_order, bits_per_pixel, bytes_per_line, colours = header[7], header[11], header[12], header[19]
if bits_per_pixel != 32:
    sys.exit(f"xwd-to-png: {bits_per_pixel} bits for each pixel, not 32")
pixels = data[header_size + colours * 12:]
rgb = bytearray(width * height * 3)
# LSBFirst (0) is B, G, R, X; MSBFirst is X, R, G, B.
red, green, blue = (2, 1, 0) if byte_order == 0 else (1, 2, 3)
for y in range(height):
    row = pixels[y * bytes_per_line:y * bytes_per_line + width * 4]
    rgb[y * width * 3 + 0:(y + 1) * width * 3:3] = row[red::4]
    rgb[y * width * 3 + 1:(y + 1) * width * 3:3] = row[green::4]
    rgb[y * width * 3 + 2:(y + 1) * width * 3:3] = row[blue::4]
pixbuf = GdkPixbuf.Pixbuf.new_from_bytes(
    GLib.Bytes.new(bytes(rgb)), GdkPixbuf.Colorspace.RGB, False, 8, width, height, width * 3
)
pixbuf.savev(sys.argv[2], "png", [], [])
