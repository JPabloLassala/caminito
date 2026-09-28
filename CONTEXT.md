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

**Frame**:
The decoration Camino draws around a window from the Marco theme: titlebar, buttons and borders, plus the invisible resize border. The shadow isn't part of the frame.
_Avoid_: decoration (for Camino's own drawing), border, titlebar (for the whole)

**SSD window**:
A window that Camino draws a frame for.
_Avoid_: decorated window

**CSD window**:
A window that draws its own titlebar and borders, such as a header-bar or libadwaita app. Camino never draws a frame for it.
_Avoid_: undecorated window (that means a window with no decorations at all)

**Palette**:
The GTK3 theme's colours that Marco themes refer to: the window background and text colours per state, plus the theme's named colours, for the default and dark variants.
_Avoid_: colour scheme (that's the portal's light/dark preference)

