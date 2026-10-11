# 27. Caminito plays its sounds through libcanberra, in its own process

Date: 2026-10-11
Status: Accepted

## Context

Ticket 170 brings Marco's sounds: the audible bell and the workspace switch sounds. Marco plays them through libcanberra's GTK integration (`bell.c:meta_bell_notify`, `workspace.c:workspace_switch_sound`), which takes the sound theme and whether event sounds are on from GTK's settings. Caminito links no toolkit (ADR 0003), and reads `org.mate.sound` itself (09).

Caminito had played no sound before. The ticket left three ways open:

- libcanberra through FFI, in Caminito's process;
- a helper process spawned for each sound, `canberra-gtk-play` or one of Caminito's own;
- a Rust crate that reads the sound theme and plays through PipeWire or PulseAudio.

The acceptance criteria ask for the player behind a seam the headless tests replace.

## Decision

- **Caminito calls libcanberra itself,** through a few `extern "C"` declarations of its own in `caminito/src/sound.rs`: a context, a property list and `ca_context_play_full`. It uses none of libcanberra's GTK integration. The developer chose this on 2026-10-11.
- **Each sound names its theme.** Caminito gives `org.mate.sound theme-name` as `canberra.xdg-theme.name` with every sound, so a change of the key is heard at the next sound with nothing to tell libcanberra.
- **The sounds are Marco's:** `bell-window-system` and `desktop-switch-left`, `-right`, `-up` and `-down`, with Marco's descriptions.
- **No sound is kept in the sound server's cache:** `canberra.cache-control` is `never`, where Marco asks for `permanent`. The cache has a sound by its name alone, so a cached bell would go on playing the last theme's after `theme-name` changes.
- **The player is a trait,** `sound::Player`, with libcanberra behind it in a Session and a recorder in the tests. Policy says which way each switch went; the compositor crate decides what plays.
- **The context is made at the first sound,** and a sound that can't be played is logged, the first as a warning and the rest at `debug`, and dropped. Caminito starts and runs the same with no sound server.

## Consequences

- The theme lookup, the sound cache and the choice of sound server are libcanberra's, as they were Marco's, so a sound theme sounds as it did under Marco.
- `caminito` links `libcanberra.so`, which `caminito-git` must depend on (173), and has more `unsafe` blocks, all in `sound.rs`.
- libcanberra's code runs in the compositor's process: a crash in it, or in the sound server's client library it loads, ends the Session.
- The first sound opens the connection to the sound server on Caminito's thread, which libcanberra waits for. Each sound's file is found and opened on that thread too, and then played by libcanberra's own.
- The bell has none of the properties Marco gives for its window (the title, the X window, where it is on the screen), so a sound server can't place it by the window.
- Marco falls back to the X server's own bell when libcanberra can't play; Caminito has no such bell and stays silent.
- The headless tests check what Caminito asks its player for, and one test calls libcanberra with its `null` driver. No test hears a sound: that's the checklist's.
- Rejected:
  - `canberra-gtk-play` for each sound (it takes the theme from GTK's settings, not from Caminito, and a process is started for every bell);
  - a helper of Caminito's own (one more binary to build and package, for what is a few calls);
  - a Rust crate over PipeWire or PulseAudio (Caminito would carry the sound theme lookup and the decoding of the sound files itself).
