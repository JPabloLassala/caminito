# msd's custom shortcuts

Type: implementation
Status: open
Milestone: M6
Blocked by: 98
Map: [Caminito v0.1](../map.md)

## What to build

A shortcut the user adds in Keyboard Shortcuts runs its command in a Session ([06](06-v01-protocol-and-integration-set.md)'s *Media keys and custom shortcuts* row, [17](17-milestone-slicing.md)).

- **The carried patch,** in `mate-settings-daemon-caminito`, gated on Caminito ([16](16-packaging.md), ADR 0011): the keybindings plugin grabs each custom shortcut through [98](98-accelerator-grabs.md)'s `GrabAccelerators` and runs its command on `AcceleratorActivated`, in place of its X11 grabs. `X11Only` is dropped for the plugin. It grabs again when a shortcut is added, changed or removed, and when `org.mate.Caminito` gets a new owner.
- **Caminito's side** is 98's.
- **The checklist:** one item, tagged M6, run in a Session.

## Acceptance criteria

- [x] `mate-settings-daemon-caminito` builds with the patch in its series, and its `makepkg` and `namcap` job passes.
- [x] The checklist has the item, and it was run in a Session: a custom shortcut runs its command; changing its keys moves the grab; removing it releases the keys to the focused app. If it can't be run where the ticket is done, say so in the notes and leave it to [106](106-packaging-and-closing-m6.md).
- [ ] On X11 MATE custom shortcuts work as before (the gate).

## Notes

- **Adding the shortcut from the capplet** needs Keyboard Shortcuts not to crash, which is [101](101-mate-control-center-caminito.md)'s. Until then the check sets the keys with `dconf`.
- **Not blocked by [99](99-msd-media-keys-and-osd.md):** the two plugins share no code, only the package's series. Whichever lands second rebases its patch number.
- **From [99](99-msd-media-keys-and-osd.md) (2026-10-07):**
  - `X11Only` isn't dropped: the daemon loads an `X11Only=true` plugin under Caminito when its file also says `Caminito=true` (`0007`), so that on Wayfire it isn't loaded at all. This plugin's file needs that line, and its patch the same gate on a display that isn't X11.
  - The two plugins are one caller to Caminito: they share msd's connection. A custom shortcut on a combination a media key has gets the same action id, and one plugin's `UngrabAccelerators` releases it for both.
  - `AcceleratorRepeated` is new, for a held key. A plugin that doesn't listen for it runs its command once for a press, where X11 runs it at every repeat.

## Resolution

One patch in `mate-settings-daemon-caminito`'s series, `0011`, at `pkgrel` 6, and a change to how Caminito matches a grab. The checklist item is *A custom shortcut runs its command*, under *Media keys*.

- **`0011`, the plugin:** under Caminito on a display that isn't X11, the manager grabs every binding's shortcut with one `GrabAccelerators` call: when `org.mate.Caminito` gets an owner, and when dconf says anything under `/org/mate/desktop/keybindings/` changed, which is a shortcut added, changed or removed. One call is out at a time, and a change in the meantime makes another after it. It calls only while the name has an owner, and never starts one (ADR 0011). A reply's actions replace the ones held, and the ones no binding has any more are released with `UngrabAccelerators`. `AcceleratorActivated` runs the command of the first binding with that action, as X11's filter runs the first that matches, with the same dialog when it can't be run. The plugin's file says `Caminito=true` beside `X11Only=true` (`0007`), as [99](99-msd-media-keys-and-osd.md) said it must. The X11 code is as it was, and is what runs on an X11 display, whatever `MATE_WM_NAME` says.
- **Caminito parses the shortcuts.** msd's own parser looks the key up in the X server's keymap, which would crash on a Wayland display, so under Caminito a binding is taken as it's written, and one Caminito can't read comes back refused.
- **The command inherits msd's environment.** X11's code rewrites `DISPLAY` for the screen the key was pressed on. There's no such screen here.
- **Caminito's side: a grab is also matched by the keysym its press types.** [98](98-accelerator-grabs.md) left it to this ticket to find out what Keyboard Shortcuts writes. It writes the keysym the press gave, lowercased, with Shift kept beside it: Shift and 1 is `<Shift>exclam`, and a keypad key with NumLock on is `KP_1`. That's read from its source, not seen: `grab_key_callback` in `capplets/keybindings/eggcellrendererkeys.c` takes the consumed modifiers out only in its GTK mode, and `mate-keybinding-properties.c` sets the X mode. The capplet doesn't run under Caminito until [101](101-mate-control-center-caminito.md). msd's X11 `match_key` finds those. 98 matched a key's first level only, so they were granted and never fired. Now a grab is of its modifiers and either keysym: the first level's, as before, or the one the press types. 98 has it under *Added by 100*.

## Decided here

- **The plugin has a D-Bus connection of its own,** which is what 99 left to this ticket. Caminito knows a caller by its connection, and the media-keys plugin grabs on the one msd shares. With one connection for both, a custom shortcut on a media key's combination would get the media key's action, and either plugin would release it for both when its own shortcut changed. With two, Caminito refuses the second to ask, and the plugin warns `Grab failed for the key binding (…)`. On X11 both grabs succeed and both plugins act. Here the one that grabbed first acts, which is the media key in a Session, since its grab is there before a shortcut is added; after Caminito starts again it's whichever plugin's call comes first.
- **Stopping the plugin closes its connection,** and Caminito releases a caller's grabs when it leaves the bus. So there's no release to send at the stop, and no late reply to release, which `0010` needs both of.
- **A held shortcut runs its command once.** The plugin doesn't listen for `AcceleratorRepeated`. X11 runs the command at every repeat, which opens a window for each: this is the one place it's not as X11 on purpose.
- **Between a change and its reply no custom shortcut fires:** the bindings are read again at the change, and get their actions from the reply. That's the time of one D-Bus call, while the user is in Keyboard Shortcuts.

## Notes from the work (2026-10-07)

- **Not run in a Session.** The machine's Session is the one from before the rename ([109](109-rename-to-caminito.md)): `MATE_WM_NAME=Camino`, with `camino-git` and `mate-settings-daemon-camino` installed, and no `org.mate.Caminito` on its bus. Running the item needs the renamed packages installed and a new login, and a hand on the keys, so it's left to [106](106-packaging-and-closing-m6.md), as the criterion allows. The second criterion is ticked on that allowance.
- **What was run instead,** with the patched msd built from the series. A program that starts the keybindings manager alone, as `test-media-keys` does the media keys', ran on a private bus with a scratch dconf, against a stand-in for `org.mate.Caminito` that records the calls and sends the signals. Neither is in the patch or the repo. Nothing was installed, and the user's dconf wasn't written. It showed:
  - no call while the name has no owner, then one `GrabAccelerators` with the shortcut, the `disabled` one not sent;
  - `AcceleratorActivated` runs the command once, and `AcceleratorRepeated` or another action runs nothing;
  - changing the keys grabs the new ones and then releases the old, and the old action runs nothing;
  - two bindings on one shortcut run one command;
  - a refused shortcut is warned about by name;
  - removing the shortcuts releases every grab;
  - a new owner of the name gets one call with everything, and no release;
  - after three changes in a row only the last keys are held;
  - stopping the manager takes its connection off the bus with msd still running, and starting it grabs on a new one;
  - with `GDK_BACKEND=x11`, on XWayland, Caminito isn't called and the X11 path starts; on Wayland without `MATE_WM_NAME` it refuses to start with `Keybindings are only supported on X11`, as stock.
- **Not run at all:** a key pressed against the real Caminito with the plugin, so `0011` and Caminito together; the plugin loaded by the daemon itself; Keyboard Shortcuts adding the shortcut ([101](101-mate-control-center-caminito.md)); an X11 MATE session, which is the third criterion, left unticked, and the checklist item's last step.
- **The package:** `makepkg -Cf` built 1.29.1-6, `namcap PKGBUILD` printed nothing, and `namcap` on the package has the two warnings it had, an empty `etc/mate-settings-daemon/xrandr` and `dbus-glib`, and no error. The CI job itself runs at the push, and the first criterion is ticked on the local run.
- **Written before the code:** Caminito's test of `<Shift>exclam`, seen to fail first: the grab was granted and the press reached the client. The review asked for the two beside it, `exclam` with no Shift named and the keypad key with NumLock, which were written after. The patch has no tests: msd has none to add to.
- **A `GrabAccelerators` call that fails,** or a reply of the wrong length, leaves the bindings with no action and the grabs of before with Caminito: those keys reach no client and run nothing, until the next change under the directory or a new owner of the name. `0010` has the same. Nothing was seen to cause one.
- **Still not grabbed:** a key by its code, `0x1f`, which Keyboard Shortcuts writes for a key with no keysym, and `<Mod2>`, `<Mod3>` and `<Mod5>`, which it doesn't write for a lock. They're refused with a log line, as in 98.
- **Two grabs can be of one press now,** as `<Shift>1` and `<Shift>exclam` from two callers: they're different combinations to the refusal, and the one grabbed first gets the press.
