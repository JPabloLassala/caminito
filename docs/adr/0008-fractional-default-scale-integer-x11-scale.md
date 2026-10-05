# 8. Output scales default to KWin's fractional rule, and the X11 scale stays an integer

Date: 2026-09-28
Status: Accepted

## Context

msd's wlrandr plugin persists and applies the output scales the user saves, including fractional ones, but it never picks a scale for an output it has no config for. So Camino's default is what a user sees until they save a layout. MATE's X11 answer, `org.mate.interface window-scaling-factor`, only knows 1 and 2. Its auto mode picks 2 on dense panels, so a typical 14" laptop panel gets 1 and everything is tiny.

GTK3, which is most of MATE, renders only at an integer scale. It has no fractional-scale-v1 or viewporter support, and it takes the largest integer `wl_output.scale` among the outputs a window is on (`gdkwindow-wayland.c:777-809`). On an output at 1.35, every GTK3 surface, mate-panel included, renders at 2 and is downsampled by the compositor. Camino's frames do the same (07). All X11 apps under XWayland share one X11 scale (ADR 0006, 11), and GTK3 on X11 takes only an integer `Gdk/WindowScalingFactor`.

## Decision

- **Camino's default output scale is KWin's `chooseScale`, ported as-is.** Target DPI is 125 for a laptop panel, 150 for an internal output with no lid, 30.5 for a TV and 96 for other monitors. At least 800 logical pixels remain, the result is clamped to 1–3 and rounded to 5% steps, anything under 1.20 becomes 1, and anything within 0.06 of an integer snaps to it. Camino's saved layout overrides it.
- **`window-scaling-factor` 1 or 2 forces that integer** on outputs with no saved config. 0, the default, uses the rule. A live change applies only to outputs no output-management client has set.
- **A carried mate-control-center patch** makes the Wayland Display capplet offer 1.00–3.00 in 5% steps, so it can show and choose any scale Camino picks.
- **The X11 scale is the largest rounded-up output scale**, an integer. XSETTINGS DPI uses msd's formulas with `org.mate.font-rendering dpi` (or 96).

## Consequences

- Laptops and dense monitors get a comfortable size with no configuration. A user who wants crisp integer scaling sets `window-scaling-factor` or picks 1.00 or 2.00 in the capplet.
- On fractional outputs, GTK3 apps, frames and X11 apps are all rendered at the next integer and downsampled, so they're slightly soft but consistent with each other. GTK4, Qt 6 and browsers render exactly.
- `window-scaling-factor` means something different from X11: it overrides the output scale rather than setting GTK's own scale.
- One more carried mcc patch.
- Rejected:
  - an integer default from MATE's X11 auto rule (tiny on typical laptop panels);
  - Mutter's heuristic as niri ports it, and cosmic-comp's (KWin's 5% steps and integer snaps were preferred);
  - always 1, as sway does;
  - a fractional X11 scale, which is cosmic-comp's default (X11 GTK apps would render at 1 with scaled fonts, unlike the same app running natively);
  - the smallest output scale, which is xwayland-satellite's choice (blurry on the dense output).
