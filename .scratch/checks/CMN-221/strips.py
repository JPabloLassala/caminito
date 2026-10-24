# Stacks the dock's strip of each frame of a sequence into one PNG, so that the pulse and the
# bounce read as a column of frames (CMN-221), without any library:
#   python3 -I strips.py <directory of PPMs> <dock height> <width to keep>
# Every NN-name-1.ppm, NN-name-2.ppm... becomes NN-name-strip.png: the bottom <dock height>
# rows and the left <width to keep> columns of each frame, one above the next.
import glob, os, re, struct, sys, zlib

def read(path):
    d = open(path, 'rb').read()
    parts = d.split(b'\n', 3)
    w, h = map(int, parts[1].split())
    return w, h, parts[3]

def write(path, w, h, rows):
    raw = b''.join(b'\x00' + row for row in rows)
    def chunk(t, c):
        x = struct.pack('>I', len(c)) + t + c
        return x + struct.pack('>I', zlib.crc32(t + c) & 0xffffffff)
    png = (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0))
           + chunk(b'IDAT', zlib.compress(raw, 6)) + chunk(b'IEND', b''))
    open(path, 'wb').write(png)

directory, dock, keep = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
sequences = {}
for path in sorted(glob.glob(os.path.join(directory, '*.ppm'))):
    m = re.match(r'(.*)-(\d+)\.ppm$', os.path.basename(path))
    if m:
        sequences.setdefault(m.group(1), []).append(path)
for name, paths in sequences.items():
    rows, width = [], None
    for path in paths:
        w, h, pix = read(path)
        width = min(keep, w)
        for y in range(h - dock, h):
            rows.append(pix[y * w * 3:y * w * 3 + width * 3])
        rows.append(b'\x80' * (width * 3))
    write(os.path.join(directory, name + '-strip.png'), width, len(rows), rows)
