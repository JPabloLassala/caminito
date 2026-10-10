# The audible bell and the workspace switch sounds

Epic: caminito-v0.1
Type: implementation
Status: needs-triage
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

- [ ] Headless tests through the fixture, written before the code, with the player behind a seam the tests replace, recording what it's asked to play: a bell with `audible-bell` on and off, and with `bell-mode` as decided; a switch in each direction with `event-sounds` on and off; `theme-name` changing.
- [ ] `audible-bell` (and `bell-mode`, if honoured) leave `UNSUPPORTED` if 147 put them there; their rows, and the switch sounds' row in the keyless section, say honoured, with the tests above.
- [ ] The checklist has an item, tagged M8: `printf '\a'` in a native and an X11 terminal, and Ctrl+Alt+arrows, with the sound theme changed in mate-control-center's *Sound* between them. Running it in a Session is the developer's, and goes to [173](173-packaging-and-closing-m8.md)'s closing run.

## Open questions

**For the developer**, to settle before building:

- **How Caminito plays sound** (ADR-worthy): libcanberra through FFI, as Marco did through GTK; a small helper process (`canberra-gtk-play`, or Caminito's own) spawned per sound; or a Rust crate that reads the sound theme and plays through PipeWire or PulseAudio. It adds a dependency, and a crash in the compositor's process if it's in-process.
- **`bell-mode`:** stays *never*, as [53](53-input-settings-translation.md) made it and `UNSUPPORTED`'s doc says, or is honoured as 09 has it (`off` silences the audible bell, whatever `audible-bell` says).

**From Marco's source**, to be settled by the implementing agent and recorded in the Done section: the event names Marco plays for the bell and for each switch direction, and whether a switch from the pager plays one.
