"""How a pinned application's new streams were first linked, from WirePlumber's s-linking log
(WIREPLUMBER_DEBUG=s-linking:5) on stdin, as CMN-209's blip-analyse.py reads it: a stream's
first link to the default, rather than to its pinned device, is one that played on the
default first, and the time to its next link is how long.

    blip.py <default sink> <pinned sink> < wireplumber.log
"""

import re
import statistics
import sys


def seconds(s):
    h, m, sec = s.split(":")
    return int(h) * 3600 + int(m) * 60 + float(sec)


default, pinned = sys.argv[1], sys.argv[2]
links = []
for line in sys.stdin:
    m = re.match(r"\S (\S+) .*link \S+ <-> (\S+) ", line)
    if m:
        links.append((seconds(m.group(1)), m.group(2)))
blips = []
for i, (time, target) in enumerate(links):
    if target == default:
        moved = next((x for x in links[i + 1:] if x[1] == pinned), None)
        if moved:
            blips.append((moved[0] - time) * 1000)
targets = [target for _, target in links]
print(f"links: {len(links)}; to the default ({default}): {targets.count(default)}; "
      f"to the pinned device ({pinned}): {targets.count(pinned)}")
if blips:
    print(f"played on the default first (ms): min {min(blips):.1f}  "
          f"median {statistics.median(blips):.1f}  max {max(blips):.1f}")
else:
    print("played on the default first: never")
