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

**Helper**:
A process that Camino itself starts and supervises: Xwayland, the palette helper and the locker. MATE components aren't helpers, because mate-session starts and restarts them.
_Avoid_: daemon, child (for MATE components)

**Session**:
A "MATE on Camino" login, from the display manager starting Camino to mate-session exiting. Camino is its parent process, and mate-session exiting ends it.
_Avoid_: desktop session (ambiguous with the X11 MATE session)

**Unsupported key**:
A gsettings key Camino reads but doesn't honour yet. It is logged once if the user has changed it from its default, and it is on the roadmap. It is not the same as a key Camino will never implement, or a key that belongs to another MATE component.
_Avoid_: ignored key, missing setting

**XWayland window**:
A window of an X11 client running on XWayland that Camino manages like any other toplevel, through Smithay's `X11Wm`.
_Avoid_: X window (ambiguous with the X protocol's window objects), legacy window

**Override-redirect window**:
An X11 menu, tooltip or popup that Camino shows in its own layer but never manages or focuses. It gets the keyboard only through an Xwayland keyboard grab.
_Avoid_: unmanaged window, X11 popup

**Camino's X connection**:
Camino's own ordinary client connection to Xwayland, alongside Smithay's window-manager connection. It publishes the desktop, work-area and frame-extent properties, and handles the bell, struts and the root messages Smithay drops. It never writes a property Smithay maintains.
_Avoid_: second window manager, X helper

**Locker**:
The helper that draws the lock screen and checks the password: `camino-locker` by default, MATE's unlock dialog on ext-session-lock. The lock itself is Camino's; the locker can crash without unlocking the session.
_Avoid_: screensaver (Camino only blanks), lock screen (for the program)

**Screensaver shim**:
Camino's implementation of `org.mate.ScreenSaver` and `org.freedesktop.ScreenSaver`, which turns MATE's lock, activity and inhibit calls into Camino's idle, blanking and lock state. mate-screensaver's daemon doesn't run under Camino.
_Avoid_: mate-screensaver (the X11 program)
