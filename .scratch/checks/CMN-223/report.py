#!/usr/bin/env python3
"""Prints what a case's DumpState says: the work area, the panels and the windows.

report.py LABEL DUMP.json. A panel's gap to its edge is read from its geometry, and a
maximised window's from where it ends, against the work area.
"""
import json
import sys

label, path = sys.argv[1:3]
dump = json.load(open(path))


def rect(r):
    return f"{r['width']}x{r['height']} at {r['x']},{r['y']}"


for output in dump["outputs"]:
    g = output["geometry"]
    print(f"{label}: output {output['name']} {g['width']}x{g['height']}, work area {rect(output['work_area'])}")
    for layer in output["layer_surfaces"]:
        p = layer["geometry"]
        gaps = {
            "left": p["x"] - g["x"],
            "top": p["y"] - g["y"],
            "right": g["x"] + g["width"] - (p["x"] + p["width"]),
            "bottom": g["y"] + g["height"] - (p["y"] + p["height"]),
        }
        print(
            f"{label}:   panel {rect(p)}, exclusive zone {layer['exclusive_zone']}, "
            f"gaps to the output's edges {gaps}"
        )
for window in dump["windows"]:
    print(f"{label}:   window '{window['title']}' {rect(window['geometry'])}")
