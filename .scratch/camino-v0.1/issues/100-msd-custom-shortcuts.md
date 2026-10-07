# msd's custom shortcuts

Type: implementation
Status: open
Milestone: M6
Blocked by: 98
Map: [Camino v0.1](../map.md)

## What to build

A shortcut the user adds in Keyboard Shortcuts runs its command in a Session ([06](06-v01-protocol-and-integration-set.md)'s *Media keys and custom shortcuts* row, [17](17-milestone-slicing.md)).

- **The carried patch,** in `mate-settings-daemon-camino`, gated on Camino ([16](16-packaging.md), ADR 0011): the keybindings plugin grabs each custom shortcut through [98](98-accelerator-grabs.md)'s `GrabAccelerators` and runs its command on `AcceleratorActivated`, in place of its X11 grabs. `X11Only` is dropped for the plugin. It grabs again when a shortcut is added, changed or removed, and when `org.mate.Camino` gets a new owner.
- **Camino's side** is 98's.
- **The checklist:** one item, tagged M6, run in a Session.

## Acceptance criteria

- [ ] `mate-settings-daemon-camino` builds with the patch in its series, and its `makepkg` and `namcap` job passes.
- [ ] The checklist has the item, and it was run in a Session: a custom shortcut runs its command; changing its keys moves the grab; removing it releases the keys to the focused app. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md).
- [ ] On X11 MATE custom shortcuts work as before (the gate).

## Notes

- **Adding the shortcut from the capplet** needs Keyboard Shortcuts not to crash, which is [101](101-mate-control-center-camino.md)'s. Until then the check sets the keys with `dconf`.
- **Not blocked by [99](99-msd-media-keys-and-osd.md):** the two plugins share no code, only the package's series. Whichever lands second rebases its patch number.
