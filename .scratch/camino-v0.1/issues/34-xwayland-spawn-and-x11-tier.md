# Xwayland spawn and the X11 tier

Type: implementation
Status: resolved
Milestone: M3
Blocked by:
Map: [Camino v0.1](../map.md)

## What to build

Xwayland running under Camino with Smithay's window manager attached, and the X11 test tier from [14](14-development-and-testing-loop.md) ready for the rest of M3 to use test-first. X11 windows aren't managed yet; that's [35](35-xwayland-windows.md).

- **The `xwayland` module (15):** Smithay's `xwayland` feature, and `XWayland::spawn` at start-up in every mode, eagerly (08). `XCURSOR_THEME` and `XCURSOR_SIZE` from `org.mate.peripherals-mouse` go through `envs`, since `spawn` clears the environment (11). No extra Xwayland arguments.
- **`Ready`:** `start_wm` with Smithay's `X11Wm`, then `DISPLAY` is set in Camino's environment for the clients it spawns. [38](38-x-connection-and-root-properties.md) moves the export after the root properties; until then it follows `start_wm` directly.
- **stderr (15):** Xwayland's stderr goes to the journal through `helpers` under its own identifier, as the palette helper's does. Xwayland itself is spawned by Smithay, not the supervisor.
- **No Xwayland (08):** if it fails to start, Camino logs it once and runs without X. The restart path is [42](42-xwayland-restart.md)'s.
- **The X11 tier (14, 15):**
  - the headless `Fixture` can spawn a real Xwayland on a free display, reaching `Ready` inside the fixture's loop;
  - `camino-test-client` gains an `x11rb` client that connects to it, reads root and window properties, creates and maps windows with given types, hints and `WM_TRANSIENT_FOR`, and sends client messages;
  - insta helpers turn property reads and event sequences into stable text;
  - CI's container installs `xorg-xwayland`, and the tier runs with the headless tier under `dbus-run-session`.

## Acceptance criteria

- [x] An X11-tier test, written before the code: the x11rb client connects, and the root has Smithay's `_NET_SUPPORTING_WM_CHECK` window.
- [x] An X11-tier test that Xwayland's `-terminate` doesn't fire while Camino's connection is attached: wait past its idle timeout with no clients, and Xwayland is still alive (11, 14).
- [x] Camino starts and runs normally with Xwayland missing from `PATH`, with one log line.
- [x] The X11 tier runs in CI with no GPU or EGL.
- [x] Nested, `xeyes` connects to Camino's `DISPLAY` (it won't be shown until 35).

## Notes

- `DISPLAY` goes into `Camino::environment`, the variables Camino gives the clients it spawns, not into the process environment: `set_var` isn't safe once the settings and D-Bus threads run. 38's test that `DISPLAY` isn't set until the root properties exist checks that map, and M5's session start-up spawns mate-session with it and exports it (08).
- Xwayland's stderr is prefixed onto Camino's stderr, as the palette helper's is, until `--session` picks the journal (`helpers::stderr_to`).
- Xwayland missing from `PATH` was checked by hand, nested under `xvfb-run` with an empty `PATH`: one warning, and Camino ran on. The nested client check used `glxgears` and `xprop -root`, since `xeyes` wasn't installed.
