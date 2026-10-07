# msd's media keys and OSD

Type: implementation
Status: open
Milestone: M6
Blocked by: 98
Map: [Camino v0.1](../map.md)

## What to build

The volume, brightness and other media keys work in a Session, and msd shows its OSD ([06](06-v01-protocol-and-integration-set.md)'s *Media keys and custom shortcuts* row, [17](17-milestone-slicing.md)).

- **The carried patches,** in `mate-settings-daemon-camino`, each gated on Camino ([16](16-packaging.md), ADR 0011):
  - the media-keys plugin grabs its keys through [98](98-accelerator-grabs.md)'s `GrabAccelerators` and acts on `AcceleratorActivated`, in place of its X11 grabs. It keeps all its actions: libmatemixer volume, launchers, eject, the power dialogs, MPRIS and rfkill. It grabs again when a key's setting changes and when `org.mate.Camino` gets a new owner;
  - `X11Only` is dropped for the plugin, so it starts under Camino;
  - the OSD window is a gtk-layer-shell surface on the overlay layer, with no keyboard;
  - `touchpad_is_present` is stubbed, so the touchpad toggle key works.
- **Camino's side** is 98's. Fix here what the plugin shows to be missing from it, such as repeat for the volume keys.
- **The checklist:** one item for 06's row, tagged M6, run in a Session.

## Acceptance criteria

- [ ] `mate-settings-daemon-camino` builds with the patches in its series, and its `makepkg` and `namcap` job passes.
- [ ] The checklist has the item, and it was run in a Session: volume up, down and mute change the volume and show the OSD above a fullscreen window; the OSD takes no focus; a key changed in Keyboard Shortcuts is grabbed at once. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md).
- [ ] On X11 MATE the media keys and the OSD work as before (the gate).

## Notes

- **Brightness and the power keys** may belong to mate-power-manager, which runs on X11 under Camino ([78](78-x11-only-autostarts-crash.md)) and can't grab keys there. Find out who handles them, and file a ticket if it isn't msd.
- **The screenshot keys** are [104](104-screenshots.md)'s.
- **The custom shortcuts** are [100](100-msd-custom-shortcuts.md)'s: another plugin over the same API.
