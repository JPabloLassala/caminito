# The Mouse and Keyboard capplets

Epic: caminito-v0.1
Type: implementation
Status: open
Milestone: M6
Blocked by: 101
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The control center's Mouse and Keyboard dialogs work in a Session, and what they change applies live ([06](06-v01-protocol-and-integration-set.md)'s *Mouse and Keyboard capplets* row, [16](16-packaging.md), [17](17-milestone-slicing.md)).

- **The carried patches,** in `mate-control-center-caminito`, gated on `MATE_WM_NAME=Caminito` (ADR 0011):
  - both capplets call `gdk_set_allowed_backends("x11")` before `gtk_init`, so they run on XWayland, where the Keyboard capplet's layout pages work. 16 chose this over `.desktop` overrides;
  - `touchpad_is_present` answers TRUE under the same gate, so Mouse shows its touchpad page.
- **Caminito's side is done:** it applies the `org.mate.peripherals-*` keys and the xkb keys live since M4 ([53](53-input-settings-translation.md), [54](54-keyboard-layouts.md)). Fix here any key the dialogs write that Caminito doesn't follow.
- **The checklist:** one item for 06's row, tagged M6, run in a Session.

## Acceptance criteria

- [ ] `mate-control-center-caminito` builds with the patches in its series, and its `makepkg` and `namcap` job passes.
- [ ] The checklist has the item, and it was run in a Session: Mouse opens with its touchpad page, and the pointer speed and tap-to-click change as they're set; Keyboard opens, a layout added there can be switched to, and the repeat rate changes. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md).
- [ ] On X11 MATE and on MATE on Wayfire both dialogs work as before (the gate).

## Notes

- **A machine with no touchpad** still shows the page under Caminito. That's 06's choice: the capplet can't ask libinput.
