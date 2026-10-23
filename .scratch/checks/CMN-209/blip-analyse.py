"""CMN-209: reads blip.sh's logs. Usage: blip-analyse.py WIREPLUMBER-LOG PROBE-LOG"""
import re
import statistics
import sys


def seconds(s):
    h, m, sec = s.split(":")
    return int(h) * 3600 + int(m) * 60 + float(sec)


# (time, target) of every stream link WirePlumber made, in order; the only
# stream is the application (paplay names its node Rhythmbox, pw-cat pw-cat).
links = []
for line in open(sys.argv[1]):
    m = re.match(r"\S (\S+) .*link \S+ <-> (\S+) ", line)
    if m:
        links.append((seconds(m.group(1)), m.group(2)))
events = [(seconds(l.split()[0]), l.split()[1]) for l in open(sys.argv[2])]

blips = []
for i, (ts, target) in enumerate(links):
    if target == "speakers":
        moved = next((x for x in links[i + 1:] if x[1] == "headset"), None)
        if moved:
            blips.append((moved[0] - ts) * 1000)
news = [e[0] for e in events if e[1] == "sink-input-new"]
acks = [e[0] for e in events if e[1] == "move-acked"]
move_latencies = []
for n in news:
    a = next((x for x in acks if x >= n), None)
    if a:
        move_latencies.append((a - n) * 1000)
# With a late probe, the first link is WirePlumber's own choice, so this is
# the margin a client has to move the stream before it is linked.
first_links = []
for n in news:
    first = next((x[0] for x in links if x[0] >= n), None)
    if first:
        first_links.append((first - n) * 1000)
targets = [x[1] for x in links]
print(f"runs: {len(news)}; links to speakers: {targets.count('speakers')}; "
      f"links to headset: {targets.count('headset')}")
for name, times in (("blip, speakers link -> headset link (ms)", blips),
                    ("sink-input-new -> move acked (ms)", move_latencies),
                    ("sink-input-new -> first link (ms)", first_links)):
    if times:
        times.sort()
        print(f"{name}: min {times[0]:.1f}  median {statistics.median(times):.1f}  "
              f"max {times[-1]:.1f}")
