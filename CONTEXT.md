# Camino

Camino is a Wayland compositor for the MATE desktop, built to be the Wayland counterpart of Marco.

## Language

**Camino**:
The Wayland compositor for the MATE desktop. It is display server, window manager and compositor in one process.
_Avoid_: Camino window manager, Camino WM

**Marco**:
MATE's X11 window manager, the behavioural and visual reference for Camino.
_Avoid_: Metacity (Marco's ancestor, only relevant for its theme format)

**Compositor**:
The single Wayland process that owns the displays and input and manages windows. On Wayland there is no separate window manager.
_Avoid_: Wayland window manager

**Carried patch**:
A change to a MATE component that Camino's packaging applies downstream and never sends upstream. It falls back to the component's normal behaviour when Camino isn't running.
_Avoid_: fork, upstream patch

**Layer surface**:
A shell surface that sits in a fixed layer of an output rather than among ordinary windows: the panel, caja's desktop, the wallpaper, notification bubbles and the screenshot area picker.
_Avoid_: panel window, dock window
