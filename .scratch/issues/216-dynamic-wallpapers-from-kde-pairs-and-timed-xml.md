# Dynamic wallpapers from KDE's pairs and timed XML

Epic: wallpaper
Type: implementation
Status: resolved
Blocked by: 210, 212, 215
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

The Dynamic kind, under "The kinds" and "The sun" in [the spec](../wallpaper/spec.md), for two of its three formats.

- **KDE's packages with `images` and `images_dark`:** the light picture by day and the dark by night, cross-fading through twilight by the sun's elevation Caminito publishes ([215](215-caminito-publishes-the-suns-position.md)).
- **GNOME's timed XML:** its pictures at its clock times, with its transitions.
- **The dialog:** a grid of the dynamic wallpapers found in `/usr/share/wallpapers` and `/usr/share/backgrounds` and those the user added, each thumbnail showing day and night halves; "Add…" for an `.xml`; a line saying whether it follows the sun or fixed times; and Positioning.
- Caminito ships no dynamic wallpaper of its own.

## Acceptance criteria

- [x] Unit tests: the pair's blend for elevations through a day; a timed XML's picture and blend for times through its cycle, with the installed `cosmos` file as a fixture; a malformed XML is refused without a crash.
- [x] [checklist.md](../../docs/testing/checklist.md) has an item: a Plasma pair turns dark as Night Light warms, and a timed XML is on its right picture for the hour.

## Done (2026-10-14)

- **The tests came after a first draft of `dynamic.rs`, not before,** at three seams. `dynamic/tests.rs` drives the pure module: a pair's blend for elevations through a day and the fixed times' night and day; the installed `cosmos` XML, copied byte for byte from mate-screensaver's `/usr/share/backgrounds/cosmos/background-1.xml` into `dynamic/cosmos.xml` so the tests run anywhere, through its cycle, its 5-second transitions, the wrap from the last picture to the first and a moment before its start; GNOME 3.38's `adwaita-timed.xml` for transitions hours long, files given per `<size>` and relative paths; nineteen malformed XMLs refused with an error and no panic; and a Plasma package as a pair. The page's and the grids' tests and a paint test of a blend follow it, and `caminito/src/tests/caminito_wallpaperd.rs` has the real drawer draw a pair light at local noon and dark at midnight, the time set through `TZ`.
- **`cosmos` has nine pictures, not eight,** so its cycle is four and a half hours and doesn't fit a day: the next midnight is in blue marble's half hour. GNOME counts a cycle from the file's start time, and so does this, in local wall-clock seconds, so a day-long XML keeps to the clock across a change of daylight saving time.
- **The pair cross-fades linearly between +6° and −6°,** the light picture above, the dark at and below civil twilight, where Night Light reaches its night temperature. It's chosen, not given: Night Light warms all afternoon from the day's highest, and a wallpaper fading all afternoon looked wrong, so the two agree about dusk and not about 15:00. Tying it to Night Light's whole curve is the developer's call.
- **Without Caminito on the bus** the drawer takes the fixed times by the local clock, night until 07:00 and from 19:00, with no twilight between. It is no sun sum, and only happens outside a Session, as in the headless tests; in a Session Caminito publishes the fixed-times sun itself when it has no location (215).
- **The settings.** An output's set gains `dynamic`, the package's folder or the XML, and the main schema `dynamics`, the timed XMLs the user added. A Dynamic with none chosen, or one that can't be read, is its colour, logged once by the drawer.
- **The drawer** reads `Sun`'s `elevation` from `org.mate.Caminito` (215) and draws again when it changes, every minute, and, for a timed XML, when its picture ends and each second of a transition, so `cosmos`'s 5-second transitions are drawn as the file gives them. The blend is in whole percent, so a frame is drawn only when it shows, each with the usual one-second fade. A part-way blend is the second picture over the first, each placed by the positioning over the colour.
- **`org.mate.background` gets the picture that shows most** (211's bridge), the same rule as the sidebar's.
- **The dialog's Dynamic pane:** a grid with the user's timed XMLs, then *MATE*'s (timed XMLs three levels into `/usr/share/backgrounds`, any other XML passed over) and *Plasma*'s (packages with `images_dark`, then any timed XML beside them). Each thumbnail is its day picture's left half and its night picture's right half: a pair's own, or a timed XML's at noon and midnight of its start day. A timed XML named `background…` is called by its folder's name, so `cosmos/background-1.xml` is *cosmos*. Under it are *Add…* for an `.xml`, which is read before it's added and says why if it isn't, a line saying whether the selected one follows the sun or the clock, and Positioning. The sidebar shows the picture that shows most now.
- **Seen working in `--demo` under Xvfb,** with clicks by `xdotool`: the grid with *cosmos* and a made-up Plasma pair, a click on each drawn in the sidebar, and the line for each. Not on Wayland, nor in a Session: running the checklist item is the developer's, or 220's.
- **Run in an Ubuntu 24.04 cloud container,** with the build's dependencies and `mate-screensaver-common` (for `cosmos`) from apt. The workspace's tests pass under `dbus-run-session`: caminito's 2079 with `/tmp/.X11-unix` made first, and 1048 in the other crates, `caminito-theme`'s goldens among them this time. One switcher test, `the_popup_is_not_drawn_before_its_delay::x11`, failed once in the full run and passed three times alone; it's a timing test of code this ticket doesn't touch. Of the ignored drawer tests, the new pair test and four others pass. `cargo clippy -p caminito-wallpaper --all-targets -D warnings` is clean, and the schema compiles with `--strict`. `tools/check`'s meson, cargo-deny, locker and extension steps weren't run; quick-xml was already in `Cargo.lock`, so it brings no new crate to deny. No PKGBUILD or Debian package was built.
- **`a_change_of_a_key_is_drawn_without_a_restart` fails here on master's drawer as on this one:** the fade stops part way, at `0x6c7987`, after 5 s. It isn't this ticket's, and wasn't looked into further.
