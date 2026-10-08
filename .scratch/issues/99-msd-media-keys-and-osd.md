# msd's media keys and OSD

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M6
Blocked by: 98
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The volume, brightness and other media keys work in a Session, and msd shows its OSD ([06](06-v01-protocol-and-integration-set.md)'s *Media keys and custom shortcuts* row, [17](17-milestone-slicing.md)).

- **The carried patches,** in `mate-settings-daemon-caminito`, each gated on Caminito ([16](16-packaging.md), ADR 0011):
  - the media-keys plugin grabs its keys through [98](98-accelerator-grabs.md)'s `GrabAccelerators` and acts on `AcceleratorActivated`, in place of its X11 grabs. It keeps all its actions: libmatemixer volume, launchers, eject, the power dialogs, MPRIS and rfkill. It grabs again when a key's setting changes and when `org.mate.Caminito` gets a new owner;
  - `X11Only` is dropped for the plugin, so it starts under Caminito;
  - the OSD window is a gtk-layer-shell surface on the overlay layer, with no keyboard;
  - `touchpad_is_present` is stubbed, so the touchpad toggle key works.
- **Caminito's side** is 98's. Fix here what the plugin shows to be missing from it, such as repeat for the volume keys.
- **The checklist:** one item for 06's row, tagged M6, run in a Session.

## Acceptance criteria

- [x] `mate-settings-daemon-caminito` builds with the patches in its series, and its `makepkg` and `namcap` job passes.
- [x] The checklist has the item, and it was run in a Session: volume up, down and mute change the volume and show the OSD above a fullscreen window; the OSD takes no focus; a key changed in Keyboard Shortcuts is grabbed at once. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md).
- [x] On X11 MATE the media keys and the OSD work as before (the gate). (Seen by the developer on 2026-10-08, in the X11 session with `mate-settings-daemon-caminito` 1.29.1-6: the volume keys work.)

## Notes

- **Brightness and the power keys** may belong to mate-power-manager, which runs on X11 under Caminito ([78](78-x11-only-autostarts-crash.md)) and can't grab keys there. Find out who handles them, and file a ticket if it isn't msd.
- **The screenshot keys** are [104](104-screenshots.md)'s.
- **The custom shortcuts** are [100](100-msd-custom-shortcuts.md)'s: another plugin over the same API.

## Resolution

Four patches in `mate-settings-daemon-caminito`'s series, `0007` to `0010`, at `pkgrel` 4, and a signal on Caminito's side. The checklist item is *The volume keys change the volume and show the OSD*, under a new *Media keys* heading.

- **`0007`, the daemon:** a plugin that's `X11Only=true` and also says `Caminito=true` is loaded on a Wayland display when `MATE_WM_NAME` is Caminito. The ticket says `X11Only` is dropped; it's kept, with this beside it, so that on Wayfire the plugin isn't loaded at all, as stock. With the line dropped, the daemon would load the module there, and the plugin's manager would put `org.mate.SettingsDaemon.MediaKeys` on the bus before anything in the plugin could refuse (ADR 0011: a patch that turns on under Wayfire is a bug). [100](100-msd-custom-shortcuts.md)'s keybindings plugin needs only the same line in its own file.
- **`0008`, the OSD:** under Caminito on a Wayland display, the window is a gtk-layer-shell surface, `mate-osd`, on the overlay layer, with no keyboard, anchored to the bottom edge with the margin that puts it where X11's code does: across the middle, in the middle of the lower half. Its exclusive zone is -1, so the panels don't move it. The compositor chooses the output, where X11's code takes the pointer's monitor, which a Wayland client can't know. It stays a `GTK_WINDOW_POPUP`: as a toplevel, GTK waited for a configure event that never comes and drew nothing.
- **`0009`, the touchpad:** `touchpad_is_present` says yes under Caminito, which applies `touchpad-enabled` to the touchpads it has ([09](09-gsettings-mapping.md)). On a machine without one the key shows the OSD and writes the setting, and nothing else happens.
- **`0010`, the plugin:** under Caminito on a display that isn't X11, the manager watches `org.mate.Caminito` and grabs every key's shortcut with one `GrabAccelerators` call: when the name gets an owner, and when a key's setting changes. One call is out at a time, and a change in the meantime makes another after it. It calls only while the name has an owner, and never starts one (ADR 0011). A reply that comes after the name got a new owner is dropped, and one that comes after the plugin was stopped has its grabs released, since msd stays on the bus. A reply's actions replace the ones held, and the ones no key has any more are released with `UngrabAccelerators`. `AcceleratorActivated` runs the key's action through the plugin's own `do_action`, so the actions are all there. The X11 code is as it was, and is what runs on an X11 display, whatever `MATE_WM_NAME` says.
- **Caminito's side: a held key repeats.** [98](98-accelerator-grabs.md) signalled a press once, so a held volume key changed the volume one step. Caminito now signals `AcceleratorRepeated` while a grabbed key is held, at the keyboard's delay and rate; 98 has the signature and the rules, under *Added by 99*. The plugin acts on it for volume up and down and their quiet variants, the ones msd's X11 code calls auto-repeatable, and not for the others: a held mute key toggles once, where X11's toggles at every repeat.

## Notes from the work (2026-10-07)

- **Not run in a Session.** The work was done inside one, on the developer's machine, with `caminito-git` r273 and `mate-settings-daemon-caminito` 1.29.1-3 installed and running. Running the item needs both replaced and a new login, and a hand on the keys, so it's left to [106](106-packaging-and-closing-m6.md), as the criterion allows.
- **What was run there instead,** with the patched msd built from the series and its two test programs run from the build directory, against the running Caminito. Nothing was installed, and the user's dconf wasn't written: the settings were a keyfile backend in a scratch directory.
  - `test-media-keys`, which starts the plugin's manager alone: `DumpState` listed 34 grabs, one for every key with a shortcut, none refused and none unparsed, so 98's parser reads every default. Setting `logout` to `<Control><Alt><Shift>F12` added a 35th at once, and setting `volume-mute` to `disabled` released its grab at once. When the program ended, the grabs were gone.
  - `test-media-window`, which shows the OSD: `DumpState` had `mate-osd` on `overlay`, 141 by 141, `keyboard_interactivity` `none`, with `focus` unchanged, and `WAYLAND_DEBUG` showed its buffers committed through the fade. With the manager's placement called, its bottom margin was 238 on an output 1234 high, which puts its top at 855, where X11's arithmetic does.
  - `test-media-keys` with `GDK_BACKEND=x11`, on XWayland, with and without `MATE_WM_NAME`: it took the X11 path, and Caminito had no grabs.
- **Not run at all:** a key pressed, so no action, no repeat and no OSD from a key; the plugin loaded by the daemon itself under Caminito, which is `0007`; the touchpad key; an X11 MATE session, which is the third criterion, left unticked, and the checklist item's last step.
- **The package:** `makepkg -Cf` built 1.29.1-4, `namcap PKGBUILD` printed nothing, and `namcap` on the package has two warnings, an empty `etc/mate-settings-daemon/xrandr` and `dbus-glib`, and no error. The CI job itself runs at the push.
- **Written before the code:** Caminito's tests of the repeats, seen to fail first. The review found that two of them, the release and another key's press, couldn't fail: the key they pressed to look for a late repeat ended the repeats itself. They wait past the next repeat now before they press it, and each was seen to fail with its ending taken out. The patches have no tests: msd has none to add to.
- **Brightness and the power keys** are mate-power-manager's, not msd's: [107](107-brightness-and-power-keys.md).
- **`<Control><Alt>Delete` and `<Control><Alt>l`,** msd's defaults for the shutdown dialog and the lock, are granted and signalled: Caminito binds neither. The lock runs `mate-screensaver-command --lock`, which calls `org.mate.ScreenSaver`, the name Caminito's Screensaver shim has ([62](62-screensaver-names.md)); what it does there wasn't run.
- **A caminito-git older than this** has no `AcceleratorRepeated`: the patched msd works with it, and a held volume key changes the volume once.
- **The review's findings** that were fixed: `0010` called Caminito after its name lost its owner, and without `NO_AUTO_START`; stopping the plugin with a call out left that call's grabs with Caminito; a late reply from the last owner of the name was taken for the new one's; the two tests above; two fixed waits had no comment. Left as they are:
  - `0009` is gated on `MATE_WM_NAME` alone, where `0008` and `0010` also ask for a display that isn't X11: an msd on XWayland under Caminito would find no touchpad among XWayland's devices either;
  - the OSD's margin is worked out from the monitor GDK gives for the window, which before it's first shown is the first one, so on an output of another height the first OSD is a little off the middle of the lower half;
  - a modifier's release doesn't end a held key's repeats, as it doesn't on X11;
  - a repeat's time is the press's plus the delay and the periods, not the clock's, so it drifts from it by what the timer is late;
  - no test has the grabber leave the bus with its key held: the same check ends the repeats as for a released grab, which has one.
- **For [100](100-msd-custom-shortcuts.md):** its plugin is in msd's process, on the same connection, so Caminito takes the two for one caller. A custom shortcut on a combination a media key has gets the media key's action id, and whichever plugin releases it releases it for both. On X11 the second `XGrabKey` of the two succeeds too, and both plugins act. 100 decides what happens here.
- **The first criterion is ticked on the local run** of what the job runs. The job runs at the push.

## The keys didn't work in a Session (2026-10-07)

- **The report:** in a Session with `caminito-git` r283, the media and volume keys did nothing.
- **The cause:** the msd installed was still 1.29.1-3, which has none of `0007` to `0010`. Its media-keys plugin file said `X11Only=true` without `Caminito=true`, so the daemon didn't load it, and Caminito's `DumpState` had no grabs. Nothing in the repo was wrong.
- **What was done:** the 1.29.1-4 package built above was installed with `pacman -U`, and the running msd was killed, which mate-session started again.
- **What it showed:** `DumpState` listed 34 grabs owned by the new msd, the volume keys among them. So the plugin is loaded by the daemon itself under Caminito, which is `0007`, not run until now.
- **Still not run:** a key pressed. The checklist item is [106](106-packaging-and-closing-m6.md)'s, as before.
- **In the Wayfire session msd is on XWayland, and loads the plugin there** (seen on 2026-10-08, with 1.29.1-6). `mate-wayland-session`'s `mate-wayland-components.sh` starts msd with `GDK_BACKEND=x11`, in a loop that starts it again when it ends, so the session's msd has an X11 display and exports `/org/mate/SettingsDaemon/MediaKeys` through the plugin's X11 code, which no patch changes and the stock package runs too. `0007`'s "on Wayfire the plugin isn't loaded at all" holds for an msd on the Wayland display: run by hand from a terminal there, the stock mpris plugin warned `Failed to call "GrabMediaPlayerKeys"`, `Object does not exist at path "/org/mate/SettingsDaemon/MediaKeys"`. The `requires X11; not loading on this display` line wasn't printed, with `--debug` or with `G_MESSAGES_DEBUG=all`; why wasn't looked into. The checklist item's expectation says what is printed.
