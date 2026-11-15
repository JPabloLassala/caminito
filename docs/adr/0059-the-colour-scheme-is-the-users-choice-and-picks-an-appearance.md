# 59. The Colour scheme is the user's choice, and it picks an Appearance

Date: 2026-11-15
Status: Accepted. Supersedes [ADR 0005](0005-msd-mirrors-mate-settings-into-gnome-schemas.md)'s "the colour scheme is derived, not translated".

## Context

ADR 0005 derives the Colour scheme from the GTK theme: the theme is the only source of truth, so to go dark the user picks a dark theme in Appearance. The user wants a light/dark switch in a panel applet and in Appearance instead.

A switch that only sets `color-scheme`, or GTK's `prefer-dark`, does nothing for most themes users actually run. Breeze, Yaru and MATE's own themes ship their dark look as a separate theme (Breeze-Dark, Yaru-dark, BlackMATE), not as a dark variant inside one theme. The window frames and the icons would also stay light.

## Decision

- **The Colour scheme is the user's choice**, and it is the input. It's a key in `org.mate.Caminito`.
- **The user keeps two Appearances, a light one and a dark one.** Each is a GTK theme, a Marco theme and an icon theme, and each is stored in `org.mate.Caminito`. The cursor and the background aren't part of it.
- **Caminito applies them.** When the Colour scheme changes, Caminito writes the matching Appearance into MATE's keys (`org.mate.interface gtk-theme` and `icon-theme`, and the Marco theme). msd's mirror copies them to GNOME as before, and it copies the Colour scheme from Caminito's key instead of working it out with the luminance test.
- **A theme change from anywhere edits the Appearance in use.** Caminito watches MATE's keys, so picking a theme in Appearance, in an unpatched tool or with `gsettings`, changes the Appearance that's active. Appearance's Theme tab gets a Light/Dark selector, so the other Appearance can be edited without switching to it.
- **Unset Appearances are filled in from a table of known pairs** (Breeze ↔ Breeze-Dark, Yaru ↔ Yaru-dark, Menta ↔ BlackMATE, …). With no known pair, the dark Appearance is the same themes, with GTK's `prefer-dark` doing the work.
- **On the first start, Caminito seeds the keys from what's active.** The current themes become the active Appearance, and the Colour scheme is worked out once with ADR 0005's luminance test. Users already on a dark theme stay dark.
- **The compositor draws from the Palette's dark variant whenever the Colour scheme is dark.** Under a dark theme like Breeze-Dark the dark variant is the same as the default, and under the `prefer-dark` fallback it's what makes the frames and the switcher go dark.
- **The switch is in the notifications applet's popup**, beside Do Not Disturb. It is hidden when Caminito isn't the compositor.

## Consequences

- There are now two sources of truth for the GTK theme: the Appearances and the Colour scheme. That's what ADR 0005 rejected. It is kept consistent by having one writer (Caminito) and by treating every theme change as an edit to the active Appearance.
- Debian's mate-control-center carries Ubuntu's `2001_appearance_support.patch`, which writes `color-scheme` from a `-dark` theme name. It's gated off under Caminito, so it doesn't compete with this key.
- A schedule, such as dark from sunset using Night Light's sun schedule, can be added later on top of the same key. It isn't part of this decision.
- Rejected:
  - a switch that only sets the Colour scheme and `prefer-dark` (it does nothing for Breeze, Yaru or MATE's themes);
  - guessing the dark theme from its name (ADR 0005 already rejected this; the table of known pairs is explicit and the user can override it);
  - adding the keys to MATE's own schemas through a mate-desktop patch, with msd applying them (it puts more in carried patches, and the logic belongs in Caminito);
  - a new applet just for the switch.
