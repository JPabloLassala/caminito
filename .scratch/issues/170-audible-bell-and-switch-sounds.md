# The audible bell and the workspace switch sounds

Epic: caminito-v0.1
Type: implementation
Status: resolved: the checklist item's run in a Session is 173's closing run
Milestone: M8
Blocked by: 169, 150
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

Caminito plays sounds, which it has never done ([05](05-v01-marco-feature-cut.md), [09](09-gsettings-mapping.md)). Marco played them through GTK's canberra integration; Caminito reads the keys itself.

- **Sound playback** from the sound theme `org.mate.sound theme-name`, by the freedesktop sound naming spec's event names, as Marco's canberra calls name them.
- **`audible-bell`:** a bell ([105](105-one-bell-event.md)'s event) plays the theme's bell sound, beside [169](169-visual-bell.md)'s flash.
- **`bell-mode`:** settled here, as 105 left it (below).
- **The workspace switch sounds:** a workspace switch plays Marco's sound for its direction ([150](150-workspace-up-down-prev-and-wrap-style.md)), gated by `org.mate.sound event-sounds`.
- **Packaging:** whatever the playback needs is a dependency of `caminito-git`, added by [173](173-packaging-and-closing-m8.md).

## Acceptance criteria

- [x] Headless tests through the fixture, written before the code, with the player behind a seam the tests replace, recording what it's asked to play: a bell with `audible-bell` on and off, and with `bell-mode` as decided; a switch in each direction with `event-sounds` on and off; `theme-name` changing.
- [x] `audible-bell` (and `bell-mode`, if honoured) leave `UNSUPPORTED` if 147 put them there; their rows, and the switch sounds' row in the keyless section, say honoured, with the tests above.
- [x] The checklist has an item, tagged M8: `printf '\a'` in a native and an X11 terminal, and Ctrl+Alt+arrows, with the sound theme changed in mate-control-center's *Sound* between them. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, to settle before building:

- **How Caminito plays sound** (ADR-worthy): libcanberra through FFI, as Marco did through GTK; a small helper process (`canberra-gtk-play`, or Caminito's own) spawned per sound; or a Rust crate that reads the sound theme and plays through PipeWire or PulseAudio. It adds a dependency, and a crash in the compositor's process if it's in-process.
- **`bell-mode`:** stays *never*, as [53](53-input-settings-translation.md) made it and `UNSUPPORTED`'s doc says, or is honoured as 09 has it (`off` silences the audible bell, whatever `audible-bell` says).

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the event names Marco plays for the bell and for each switch direction, and whether a switch from the pager plays one.

## Done (2026-10-11)

The player is `caminito/src/sound.rs`; the bell plays from `caminito/src/bell.rs` and the switches from the refresh pass (`refresh_sounds`). The tests are `caminito/src/tests/sounds.rs`, `caminito/src/tests/bell.rs` and `caminito-policy/src/tests/workspace_moves.rs`.

- **The developer's answers, given on 2026-10-11:** libcanberra through FFI, in Caminito's process ([ADR 0027](../../docs/adr/0027-caminito-plays-its-sounds-through-libcanberra.md)), and `bell-mode` honoured as 09 has it: `off` silences the audible bell whatever `audible-bell` says, and `on` and `custom` are both on. It's read into `Bell::audible` with `audible-bell`, left the *never* list in `UNSUPPORTED`'s doc, and has a row in the parity table's keyless section. Neither key was in `UNSUPPORTED`.
- **From Marco's source (1.29.0):** the bell is `bell-window-system` (`bell.c:296`). A switch is `desktop-switch-left`, `-right`, `-up` or `-down` (`workspace.c:345-352`), by where the new workspace is in the grid and not by the key: left or right where the columns differ, up or down only within a column, so a switch that wraps past the edge sounds as the other way. Marco plays it in `meta_workspace_activate_with_focus` (`workspace.c:382-383`), so a switch from the pager plays one, as does every other: an activation, a move of a window to the workspace beside, and the active workspace's removal (`screen.c:1153-1154`). Caminito does the same.
- **Policy notes each switch's direction** (`Policy::take_workspace_switches`), in the grid as it was when the switch came, and the compositor plays them in its refresh pass. With `dynamic-workspaces`, the active workspace following its windows down is no switch; its removal is one.
- **Written before the behaviour:** the seam (`sound::Player`, the tests' `Recorder`) and the tests went in first, and ran with the bell's and the switches' calls and `bell-mode`'s reading taken out: 10 failed (the bell on both kinds, `bell-mode` `custom`, the key changes, the switches by direction, from the pager and at a removal, and the translation's `bell-mode` `off`). The tests for what stays silent passed then too, as they must. The tests for `dynamic-workspaces` and for libcanberra itself went in after the code review, with their code.
- **`event-sounds` doesn't gate the bell, which differs from Marco.** Marco's bell goes through GTK's canberra context, which `event-sounds` turns off, so Marco's audible bell is silent with it off. 09 and this ticket gate only the switch sounds, and Caminito follows them. For the developer to confirm; the change would be one condition in `ring_bell`.
- **No sound is cached, where Marco asks for `permanent`.** The sound server's cache has a sound by its name alone, so a cached bell would go on playing the last theme's after `theme-name` changes. This is from what I know of libcanberra's PulseAudio driver, not from a run: the checklist's step 4 is the check.
- **The bell has none of Marco's window properties** (title, position), and there's no fallback to an X bell (ADR 0027).
- **Not heard here:** the OrbStack machine has no sound server. One test calls libcanberra with its `null` driver, which covers the FFI's calls and not a sound. The checklist's *The bell and the workspace switches sound* is a Session's, for 173.
- **Packaging:** `caminito` links `libcanberra.so` now. `tools/check --deps` installs it; `caminito-git`'s `depends` is 173's, and until then the package builds only because the MATE packages it depends on bring libcanberra in.
- **The tests run,** in the OrbStack machine `camino`, filtered by name and not the whole suite: `caminito-policy`'s `workspace_moves` and `dynamic_workspaces`; `caminito`'s `sound`, `tests::sounds`, `tests::bell`, `tests::parity`, `tests::workspace_moves`, `tests::dynamic_workspaces`, `tests::ext_workspace`, `tests::settings`, `xsettings` and `settings::translate::tests`; clippy with `-D warnings` on both crates, and rustfmt. The `::x11` bell tests ran there.
