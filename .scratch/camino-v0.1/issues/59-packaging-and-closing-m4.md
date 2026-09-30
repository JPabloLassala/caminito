# Packaging and closing M4

Type: implementation
Status: open
Milestone: M4
Blocked by: 52, 54, 56, 57, 58
Map: [Camino v0.1](../map.md)

## What to build

M4's code installed through `camino-git` (17: packaging keeps up), its checklist items, and the closing commit.

- **`camino-git` (16):** gains the runtime dependencies the tty backend links or loads: seatd (libseat), libinput, mesa (GBM, EGL), systemd-libs (udev), and whatever Smithay's new features need. `namcap` reports nothing unexplained.
- **Checklist items tagged `M4`** in `docs/testing/checklist.md` (17), each with steps, the expected result and the debug tool:
  - a GTK3 window dragged across a 1.0 and a 2.0 output (13's visual part);
  - hot-plugging a monitor;
  - the scale picked for each of the developer's monitors.
- **Closing:** M4's exit from [17](17-milestone-slicing.md) on the installed package, as `caminotest` on a spare tty, with the results of every `M1` to `M4` checklist item that still applies recorded in a `Close M4 On the metal` commit.

## Acceptance criteria

- [ ] `makepkg -si` in `packaging/arch/camino-git/` installs a `camino` that runs on a tty, and the packaging job is green in CI.
- [ ] The three checklist items are written.
- [ ] Closing M4: as `caminotest` on a spare tty, bare `camino` drives both of the developer's monitors at their default scales; `wlr-randr` changes modes, scales and positions; mouse, touchpad and keyboard-layout settings changed with `gsettings` apply live; switching VT away and back works; CI is green; the checklist results are in a `Close M4 On the metal` commit.
