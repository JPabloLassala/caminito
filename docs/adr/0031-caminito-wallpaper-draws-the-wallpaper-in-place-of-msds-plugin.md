# 31. caminito-wallpaper draws the wallpaper, in place of mate-settings-daemon's plugin

Date: 2026-10-12
Status: Accepted

## Context

The developer wants a wallpaper per output, slideshows, a picture of the day from online sources, and dynamic wallpapers that follow the time of day, set from a dialog like Plasma's.

Today mate-settings-daemon's background plugin draws the wallpaper: one layer surface per output in the background layer, all showing the one picture `org.mate.background` names. Two carried patches make it work under Caminito (it draws under caja's desktop, and its surfaces don't pile up on hot-plug). Caminito itself only orders the surface behind caja's desktop. The plugin draws through mate-desktop's `MateBG`, which has one wallpaper for every output, and timed XML files as its only notion of change. caminito-locker reads the same keys for the lock screen.

## Decision

- **A new Caminito program, caminito-wallpaper, draws the wallpaper.** It is a layer-shell client with one surface per output in the background layer, as the plugin's were. It also decodes the pictures, keeps the timers and does the downloads.
- **mate-settings-daemon's background plugin is off in a Session.** Under X11 MATE and anywhere else it is untouched.
- **The settings are Caminito's own**, in gsettings under `org.mate.Caminito`, per output. `org.mate.background` stops being the store. Two one-way bridges keep its readers and writers working: the primary output's current picture is written to `picture-filename`, and a write to that key by anything else becomes the picture of every output.
- **The compositor draws none of it.** It asks nothing more of the wallpaper than of any layer surface, and gives caminito-wallpaper the sun's position, which it already works out for Night Light.

## Consequences

- MATE's Appearance → Background tab has no effect in a Session, so a carried patch hides it there.
- caminito-locker can no longer take the wallpaper from `org.mate.background` alone if each output's lock screen is to show that output's wallpaper. It asks caminito-wallpaper, or reads what it publishes.
- The two carried patches to the plugin stay while the plugin can still be turned on in a Session, and can be dropped once it can't.
- A crash of caminito-wallpaper costs the wallpaper and nothing else. The session restarts it.
- Rejected:
  - patching mate-settings-daemon and mate-desktop for per-output keys, daily pictures and day cycles (a large patch in C to carry across two packages and every MATE release);
  - drawing the wallpaper in Caminito's scene (per-output wallpapers and cross-fades would be cheap, but picture decoding, timers and network requests would move into the compositor, where a fault takes the session with it).
