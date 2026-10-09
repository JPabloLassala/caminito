# 23. Caminito owns Night Light through the gamma tables, and steps back per output for wlr-gamma-control

Date: 2026-10-09
Status: Accepted

## Context

The developer wants Night Light as Plasma has it, and a gradual schedule that follows the sun in place of macOS's True Tone, which needs a light sensor. MATE has no Night Light of its own, and no msd plugin or setting for it.

On Wayland, the usual way to get one is for the compositor to offer `wlr-gamma-control` and let the user run gammastep or wlsunset. Neither tool has the gradual curve the developer wants, and MATE would have no settings for it. Ticket 06 left `wlr-gamma-control` out of v0.1 because nothing in its scope binds it.

Smithay at Caminito's pin has no gamma API, but `DrmDevice` gives Caminito the device itself. DrmCompositor's commits never touch a CRTC's `GAMMA_LUT`, so a table Caminito sets stays across frames and modesets. niri sets gamma this way and serves `wlr-gamma-control`.

## Decision

- **Night Light is Caminito's own.** Caminito works out the temperature from the schedule and writes it as each output's gamma table: atomic `GAMMA_LUT` where the CRTC has it, and the legacy gamma call otherwise. Its settings are `org.mate.Caminito` keys.
- **It goes through the gamma table, not a shader.** Screenshots and screencasts stay true to the content, it costs nothing per frame, and it leaves direct scanout alone. The nested run has no gamma table, so Night Light there keeps its state, shown in `DumpState`, and tints nothing.
- **Caminito also serves `wlr-gamma-control`, as a privileged global** (06's rule: not to clients in a `wp_security_context_v1` sandbox), and only on the tty backend.
- **A gamma client wins its output.** While a client holds an output's table, Night Light steps back from that output alone. When the client releases the table or exits, Night Light fades back in on that output. The other outputs are never affected.

## Consequences

- Night Light can't be seen in a nested run, so its tests read its state rather than pixels, and the tints are checked by eye on the tty.
- Caminito resets each CRTC's table when it takes the device, because a previous compositor's table survives `DrmDevice::new`. It applies the table again after a session resume, a modeset or DPMS on, and never while the session is paused.
- Rejected:
  - `wlr-gamma-control` alone, with a third-party tool (no gradual curve, no MATE settings);
  - a shader pass after rendering (a pass every frame, it breaks direct scanout, and `wlr-gamma-control` would need the gamma table anyway);
  - Night Light stepping back from every output when a gamma client holds one.
