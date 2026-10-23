# 42. Caminito makes the Screencasts and a portal backend of its own serves them

Date: 2026-10-23
Status: Accepted

## Context

The developer wants macOS's capture shortcuts: Screenshots of an output, a window or an area, and a Screen recording of an output or an area, through `caminito-capture`. Caminito already serves the ext-image-capture protocols, which mate-screenshot uses for stills, but nothing streams: no PipeWire, no xdg-desktop-portal ScreenCast backend, so no program can record or share the screen. The GTK portal that `mate-portals.conf` picks has no ScreenCast outside GNOME.

## Decision

- **GNOME's split.** Caminito makes each Screencast as a PipeWire stream, behind a private D-Bus API in the manner of `org.gnome.Mutter.ScreenCast`. `xdg-desktop-portal-caminito`, a GTK4 process, implements the portal's ScreenCast (and later Screenshot) on top of it, with the dialog that chooses a source and the panel indicator that shows the screen is being shared.
- **`caminito-capture` calls the private API, not the portal,** as GNOME's own recorder does. The user has already shown what to record with the Capture picker, a second dialog would undo the one-keypress feel, and the portal can't express an area.
- **The Capture picker is an Interaction of Caminito's,** drawn in the scene like the Window menu (ADR 0026) and asked for over D-Bus. Caminito already knows the windows' geometry and stacking, which no public protocol hands a client.
- Stills stay on ext-image-copy-capture.

## Consequences

- Only unsandboxed clients may use the private API, as with the other privileged globals; sandboxed apps go through the portal and its dialog.
- Streams start as shm copies; dmabuf comes later, from Caminito's renderer, which a backend outside the compositor couldn't give.
- Rejected:
  - Everything inside Caminito: a GTK dialog crashing would take the session with it.
  - A backend that pulls frames through ext-image-copy-capture, as xdg-desktop-portal-wlr does through screencopy: an extra copy per frame, and no way to an area or to dmabuf from the renderer.
  - `caminito-capture` drawing its own picker as a layer surface: it would need a new private protocol for window geometry anyway.
