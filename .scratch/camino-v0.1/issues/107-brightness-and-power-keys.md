# The brightness and power keys do nothing in a Session

Type: implementation
Status: open
Milestone: none yet: the developer's to place. 17's M6 exit names brightness.
Blocked by: none
Map: [Camino v0.1](../map.md)

## What to build

In a Session the brightness keys, the sleep and hibernate keys, the keyboard backlight keys and the battery key do nothing, unless an XWayland window has the focus. Filed by [99](99-msd-media-keys-and-osd.md), which was asked who handles them: it isn't msd.

- **They're mate-power-manager's.** It grabs them itself with `XGrabKey` on the root window (`gpm-button.c`): `XF86PowerOff`, `XF86Sleep`, `XF86Suspend`, `XF86Hibernate`, `XF86MonBrightnessUp` and `Down`, `XF86KbdBrightnessUp` and `Down`, `XF86KbdLightOnOff`, `XF86ScreenSaver`, `XF86Battery` and the lid's. It changes the backlight itself, through its `mate-power-backlight-helper`, and shows its own OSD.
- **msd's media-keys plugin has none of them.** Its `power` key is `<Control><Alt>Delete`, which opens mate-session's shutdown dialog, and it has no brightness key at all. So [06](06-v01-protocol-and-integration-set.md)'s *Media keys and custom shortcuts* row and 17's "volume, brightness and custom shortcuts from msd" put brightness in the wrong program.
- **Under Camino it runs on XWayland** ([78](78-x11-only-autostarts-crash.md), ADR 0014), where its grabs see a key only while an XWayland window has the keyboard, as that ADR's consequences already say.

**What to decide:** how the keys reach it, or what stands in for it. Some ways:

- a carried patch to mate-power-manager, a seventh patched component, that grabs its keys through `GrabAccelerators` ([98](98-accelerator-grabs.md)) as msd's media-keys plugin now does, with its OSD on gtk-layer-shell if it stays on XWayland, where a layer surface isn't possible, or runs on Wayland, where it crashed in `XKeysymToKeycode` (78);
- Camino binds the brightness keys itself and calls mate-power-manager's `org.mate.PowerManager.Backlight` on the session bus, which needs no patch, and leaves the OSD and the other keys;
- leave them to M7, which decides what Camino's idle and lock do with mate-power-manager anyway.

## Acceptance criteria

- [ ] The decision is recorded, with the milestone it's for.
- [ ] If it's code: in a Session, with a native Wayland window focused, the brightness keys change the backlight on a laptop.

## Notes

- **The power button itself** is logind's while nothing inhibits it. mate-power-manager takes logind's `handle-power-key`, `handle-suspend-key` and `handle-lid-switch` inhibitors and acts on its own settings. On XWayland it holds the inhibitors and may never see the key, so the button may do nothing in a Session. Not checked.
- **What was looked at:** the strings of Arch's `mate-power-manager` 1.28 binary, which has `XGrabKey` and the three inhibitors, and msd 1.29.1's `acme.h`, the table of the keys its plugin handles. mate-power-manager's source wasn't read here: the list of its keys is from memory of `gpm-button.c` and is to be checked against it.
