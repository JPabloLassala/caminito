# The lock screen shows each output's wallpaper

Epic: wallpaper
Type: implementation
Status: resolved
Blocked by: 210
Spec: [Wallpaper](../wallpaper/spec.md)

## What to build

"The lock screen" in [the spec](../wallpaper/spec.md). caminito-locker reads `org.mate.background` today, which now carries the primary output's picture alone.

- **The drawer writes a small state file** whenever an output's picture changes: per output, a picture the locker can show as it is, the positioning and the colour. For a dynamic wallpaper mid-fade it is the nearer of the two pictures.
- **caminito-locker reads the file** and shows each output's own wallpaper. Reading it never waits on the drawer, which may have crashed or hung.
- **With no file, or an output the file doesn't have,** the locker does as it does today.

## Acceptance criteria

- [x] The file is written whole or not at all, and a test reads it while it is being replaced.
- [x] caminito-locker's existing tests pass, with new ones for a file with two outputs, a missing file and a file naming a picture that is gone.
- [x] [checklist.md](../../docs/testing/checklist.md) has an item: with a different wallpaper per output, each lock screen shows its own, and with the drawer killed the lock still comes up.

## For the implementing agent to settle

Recorded in the Done section: where the file is and its format.

## Done (2026-10-15)

- **The file is `$XDG_RUNTIME_DIR/caminito-wallpaper/lock-screen`,** in the session's runtime directory, so it goes with the session and a first lock after a reboot does as before. It is a GLib key file, which both the drawer (`glib::KeyFile`) and the locker (`GKeyFile`) read and write with escaping done for them. One group per output, named by its connector, with `make`, `model`, `x`, `y`, then `picture-filename` (empty for a colour), `picture-options` and `primary-color` under `org.mate.background`'s names and values. `caminito-wallpaper/src/lock_screen.rs` owns the format; its module doc has an example.
- **The locker tells a monitor by make, model and place, not connector.** GTK 3 has no public connector (`gdk_monitor_get_connector` is GTK 4's; GTK 3.24 keeps it private). Make and model come from the same `wl_output.geometry` the drawer reads, and Smithay sends that geometry's x,y and xdg-output's logical position from the same location, so they match at any scale. Two mirrored monitors of the same make and model would both take the first group; that is left as it is.
- **Written whole:** `glib::file_set_contents`, a temporary file renamed over the last. The unit test reads in a loop while a thread replaces a 64 KiB file 400 times, and fails on a plain `std::fs::write` (tried: it read 0 bytes). The drawer writes after every refresh, only when the text changed, and refreshes when an output goes so its group goes too. A dynamic wallpaper's picture is `Drawn::nearer`, the one `org.mate.background` gets (216), so it can trail the desktop by up to the drawer's 60-second tick.
- **Fallbacks.** With no file, no group for the monitor, a group missing a key, or a picture that is gone, that monitor's lock window does as before: MateBG from `org.mate.background`. A gone picture could have shown the group's colour instead; doing as before keeps one fallback. mate-screensaver's `picture-filename`, when its file exists, still wins over the file, as it won over the desktop's before.
- **Tests:** `lock_screen`'s five unit tests; the real drawer writing each output's colour, make, model and place, then a picture and its positioning on a change (`the_lock_screens_file_has_what_each_output_shows`); and the real locker with a two-output file, with no file and with a file lacking one output, and with a gone picture, checked by each output's pixels. No drawer runs in the locker's tests, as if it had crashed.
- **Run in an Ubuntu 24.04 cloud container,** with the build's dependencies from apt and gtk-session-lock 0.2.0 built from source (Ubuntu has none). Under `dbus-run-session` caminito's 2085 tests pass, and with `CAMINITO_WALLPAPERD` and `CAMINITO_LOCKER` set the 15 ignored drawer and locker tests pass, as do the helpers' two meson tests. The other crates' tests pass but `caminito-theme`'s goldens, whose Yaru images differ by a few pixels here on the base branch too. `cargo clippy -p caminito -p caminito-wallpaper --all-targets -D warnings` is clean with `manual_range_contains` allowed for the three `caminito-theme` errors this toolchain has on the base too, and `rustfmt` differs only in two files the base has unformatted. cargo-deny, the extension and the packages weren't run or built, and nothing new is a dependency. The checklist item is the developer's, or 220's.
