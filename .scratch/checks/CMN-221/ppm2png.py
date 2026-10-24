# Turns the PPMs the Dock applet's tests leave (CMN-219, CMN-221) into PNGs beside them, without any
# library: python3 -I ppm2png.py shot.ppm...
import sys, zlib, struct
def convert(src, dst):
    d = open(src, 'rb').read()
    parts = d.split(b'\n', 3)
    w, h = map(int, parts[1].split())
    pix = parts[3]
    raw = b''.join(b'\x00' + pix[y*w*3:(y+1)*w*3] for y in range(h))
    def chunk(t, c):
        x = struct.pack('>I', len(c)) + t + c
        return x + struct.pack('>I', zlib.crc32(t + c) & 0xffffffff)
    png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw, 6)) + chunk(b'IEND', b'')
    open(dst, 'wb').write(png)
for a in sys.argv[1:]:
    convert(a, a[:-4] + '.png')
