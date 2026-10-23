"""The server's sink inputs, from pactl's JSON, one line each: index, application, sink's
name, each channel's percent, whether it's muted and whether it's corked. With an
application's name, only its.

    streams.py [<application.name>]
"""

import json
import subprocess
import sys

sinks = {s["index"]: s["name"] for s in json.loads(
    subprocess.run(["pactl", "-f", "json", "list", "sinks"], capture_output=True, text=True).stdout)}
for stream in json.loads(subprocess.run(["pactl", "-f", "json", "list", "sink-inputs"],
                                        capture_output=True, text=True).stdout):
    app = stream["properties"].get("application.name", "?")
    if len(sys.argv) > 1 and app != sys.argv[1]:
        continue
    volumes = " ".join(v["value_percent"] for v in stream["volume"].values())
    role = stream["properties"].get("media.role", "")
    print(stream["index"], app, sinks.get(stream["sink"], stream["sink"]), volumes,
          "muted" if stream["mute"] else "unmuted", "corked" if stream["corked"] else "playing", role)
