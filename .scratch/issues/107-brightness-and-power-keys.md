# The brightness and power keys do nothing in a Session

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in, and the check on a laptop is M7's
Milestone: M7, the developer's decision on 2026-10-08. 17's M6 exit named brightness, and no longer does.
Blocked by: none
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

In a Session the brightness keys, the sleep and hibernate keys, the keyboard backlight keys and the battery key do nothing, unless an XWayland window has the focus. Filed by [99](99-msd-media-keys-and-osd.md), which was asked who handles them: it isn't msd.

- **They're mate-power-manager's.** It grabs them itself with `XGrabKey` on the root window (`gpm-button.c`): `XF86PowerOff`, `XF86Sleep`, `XF86Suspend`, `XF86Hibernate`, `XF86MonBrightnessUp` and `Down`, `XF86KbdBrightnessUp` and `Down`, `XF86KbdLightOnOff`, `XF86ScreenSaver`, `XF86Battery` and the lid's. It changes the backlight itself, through its `mate-power-backlight-helper`, and shows its own OSD.
- **msd's media-keys plugin has none of them.** Its `power` key is `<Control><Alt>Delete`, which opens mate-session's shutdown dialog, and it has no brightness key at all. So [06](06-v01-protocol-and-integration-set.md)'s *Media keys and custom shortcuts* row and 17's "volume, brightness and custom shortcuts from msd" put brightness in the wrong program.
- **Under Caminito it runs on XWayland** ([78](78-x11-only-autostarts-crash.md), ADR 0014), where its grabs see a key only while an XWayland window has the keyboard, as that ADR's consequences already say.

**What to decide:** how the keys reach it, or what stands in for it. Some ways:

- a carried patch to mate-power-manager, a seventh patched component, that grabs its keys through `GrabAccelerators` ([98](98-accelerator-grabs.md)) as msd's media-keys plugin now does, with its OSD on gtk-layer-shell if it stays on XWayland, where a layer surface isn't possible, or runs on Wayland, where it crashed in `XKeysymToKeycode` (78);
- Caminito binds the brightness keys itself and calls mate-power-manager's `org.mate.PowerManager.Backlight` on the session bus, which needs no patch, and leaves the OSD and the other keys;
- leave them to M7, which decides what Caminito's idle and lock do with mate-power-manager anyway.

## Acceptance criteria

- [x] The decision is recorded, with the milestone it's for.
- [ ] If it's code: in a Session, with a native Wayland window focused, the brightness keys change the backlight on a laptop.

## Notes

- **The power button itself** is logind's while nothing inhibits it. mate-power-manager takes logind's `handle-power-key`, `handle-suspend-key` and `handle-lid-switch` inhibitors and acts on its own settings. On XWayland it holds the inhibitors and may never see the key, so the button may do nothing in a Session. Not checked.
- **What was looked at:** the strings of Arch's `mate-power-manager` 1.28 binary, which has `XGrabKey` and the three inhibitors, and msd 1.29.1's `acme.h`, the table of the keys its plugin handles. mate-power-manager's source wasn't read here: the list of its keys is from memory of `gpm-button.c` and is to be checked against it. (Checked against 1.28.1's in the work: it's that list.)

## Decision (2026-10-08)

**The first way, in M7.** The developer chose a carried patch to mate-power-manager, `mate-power-manager-caminito`, and placed the ticket in M7.

- **Why the patch:** it's msd's way (99's `0010`), on 98's API, which already repeats a held key. It keeps all of mate-power-manager's keys, and the policy and OSD behind them. If Caminito bound brightness itself, it would copy mate-power-manager's steps, show no OSD, and leave the other keys dead.
- **Why M7:** M7 already decides what Caminito's idle and lock do with mate-power-manager: its lock on suspend, its logind inhibitors and its idle dimming (ADR 0014). 17's M6 exit drops brightness, and its M7 scope and exit name the keys.

## Resolution

One carried patch in `packaging/arch/mate-power-manager-caminito`, a seventh patched package, on Arch's mate-power-manager 1.28.1, with `pkgrel` 1. ADR 0011 and ADR 0014 are qualified, and the checklist item is *The brightness keys change the backlight with a Wayland window focused*, under *Media keys*, tagged M7.

- **`0001`, `gpm-button.c`:** when `MATE_WM_NAME` is Caminito, the button watches `org.mate.Caminito` and grabs its eleven keys with one `GrabAccelerators` call each time the name gets an owner. It makes no X11 grab. The keys are those of its X11 grabs, checked against 1.28.1's `gpm_button_init`: `XF86PowerOff`, `XF86Suspend` (which it takes for hibernate, as on X11), `XF86Sleep`, `XF86Hibernate`, `XF86MonBrightnessUp` and `Down`, `XF86ScreenSaver`, `XF86Battery`, `XF86KbdBrightnessUp` and `Down`, and `XF86KbdLightOnOff`. The lid isn't a key: UPower reports it, as before.
  - `AcceleratorActivated` presses the key's button through `gpm_button_emit_type`, so the 125 ms check for duplicates is the X11 one, and gpm-manager, gpm-backlight and gpm-kbd-backlight act on it unchanged.
  - `AcceleratorRepeated` presses it again only for the four keys that step a backlight. On X11 every held key repeats, at most every 125 ms. Here a held power, sleep or toggle key acts once.
  - The grabs are released when the button is finalised. A reply that comes after the name got a new owner is dropped.
  - The gate is the variable alone, not the display as in msd's patches: mate-power-manager is on XWayland under Caminito (ADR 0014), so its display is an X11 one there too. No calls are made unless the name has an owner, and none starts it (ADR 0011).
  - **Placed to apply to upstream's master too:** master's Wayland support (`c8f6818`, August 2026, no tag yet) puts the X11 grabs under `#ifdef HAVE_X11`. The watch starts beside the timer and `gpm_button_xevent_key` skips its grab, so the series applies there as it is and the weekly patches-on-head job stays green. On master's Wayland path the keys are left to the compositor, so the patch is what makes them work there as well.
- **The package:** Arch's PKGBUILD with the smallest diff, the tag's git source and what building from it needs (`git`, `glib2-devel` for `glib-genmarshal`, `mate-common`, `yelp-tools`), and the patch. It's in `packaging.yml`'s order after `mate-utils-caminito`, in the weekly matrix, in `tools/bootstrap` and the README, and in `caminito-git`'s `depends`. `caminito-desktop` still names `mate-power-manager`, which the package provides, as it does for mcc and mate-utils.

## Notes from the work (2026-10-08)

- **Written before the code:** [107-checks/button-test.c](107-checks/button-test.c), run by [107-checks/run.sh](107-checks/run.sh) on a mate-power-manager checkout. It builds the patched `gpm-button.c` with a stand-in for `org.mate.Caminito` on a private bus, on Xvfb, and reads `button-pressed`. Thirteen checks: the grab when the name appears, with the eleven accelerators in order; no X11 grab; each key's button; nothing for a refused key, an unknown action or action 0, or for a signal from another sender; the repeats; no repeat for the other keys; the duplicate check; the grab again for a new owner, whose ids alone work; the release when the button goes; and without `MATE_WM_NAME`, no call and the X11 grabs as before. It failed first on stock 1.28.1 (no grab), and all thirteen pass on 1.28.1 with the patch. Taking out the repeat check, the grab for a new owner, the release, the test of action 0 or the skipped X11 grab each makes one fail.
- **What the test found:** the release was never sent. Without a reply callback, `g_dbus_connection_call` only queues the message, and the button held the last reference to the session connection. In mate-power-manager the manager's other objects keep the bus open, but the call now has a callback that holds the connection until the reply.
- **On upstream's master:** the patch applies, and the Caminito checks pass. The last X11 check fails there because the test's empty `config.h` has no `HAVE_X11`, under which master grabs. That's the test's, not the patch's.
- **Not built as a package:** the work was done in an Ubuntu container, where `makepkg` and `namcap` aren't available. The patched `gpm-button.c` compiled in mate-power-manager's own build, configured as the PKGBUILD configures it, with no warning of its own (two deprecation warnings that are upstream's). Building the rest stopped on Ubuntu's mate-desktop 1.26, which has no `mate-image-menu-item.h`. The packaging job builds the package at the push.
- **Arch's current PKGBUILD wasn't reachable:** `gitlab.archlinux.org` was refused by the session's proxy. The PKGBUILD is the last one Arch published on GitHub (1.27.0, in `svntogit-community`), moved to 1.28.1's git tag and SPDX licence. Diff it against Arch's 1.28.1 PKGBUILD and carry over what differs.
- **The applets** are built against `mate-panel-caminito`'s applet library, `libmate-panel-applet-4.so.2`, as the dictionary applet of `mate-utils-caminito` is. Whether 1.28.1's brightness and inhibit applets build against master's applet API wasn't checked, since the build here stopped before them. If they don't, the job says so.
- **Not run:** a key pressed in a Session, on any machine. So no backlight changed, no OSD was seen, and the second criterion stays unticked for M7's closing run.
- **A key with a modifier held does nothing under Caminito** (found by the review). The X11 grab is with `AnyModifier`, so Shift+brightness up steps the backlight on X11. Caminito matches an accelerator's modifiers exactly, except the locks (98), so `XF86MonBrightnessUp` fires only with none held. Left as it is: matching it would take a grab of each of the sixteen combinations of Shift, Control, Alt and Super for each key, or an any-modifier form of `GrabAccelerators`, which is 98's API to change. Holding a modifier with these keys is rare.
- **With `MATE_WM_NAME=Caminito` and no owner of `org.mate.Caminito` ever,** the keys do nothing, even with an XWayland window focused, where stock's X11 grab would see them. That's ADR 0011's start-up gate. Caminito owns the name before mate-session starts, so this is a Caminito that couldn't reach the bus.
- **The OSD** is mate-power-manager's own popup, an XWayland window. It's placed on the monitor under the pointer as XWayland knows it, which is where the pointer last was over an XWayland window. Not seen.
- **The backlight itself:** on XWayland there's no RandR backlight property, so mate-power-manager falls back to `mate-power-backlight-helper` through polkit. That's its own path, and isn't checked here.
- **The power button** (the ticket's first note) isn't changed by the patch: it's logind's while mate-power-manager holds the inhibitor, and `XF86PowerOff` now reaches mate-power-manager through Caminito. Whether the kernel's key reaches Caminito as `XF86PowerOff` while logind has it wasn't checked.
