# caminito-locker: MATE's unlock dialog on a lock surface

Epic: caminito-v0.1
Type: implementation
Status: open: the code is in, the first two criteria wait on CI, and the checklist item is 136's
Milestone: M7
Blocked by: 125
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

`helpers/caminito-locker`, a C GTK3 program on gtk-session-lock, ported from mate-screensaver's `mate-screensaver-dialog.c`, `gs-lock-plug.c` and `gs-auth-pam.c` (ADR 0007, [12](12-screen-locking.md)). A lock shows MATE's dialog, and the password unlocks.

- **The port:** `GSLockPlug` stops being a `GtkPlug` and is a widget in a gtk-session-lock window. Every monitor has a lock window, and the dialog is on the one under the pointer.
- **The dialog** is MATE's: `lock-dialog-<lock-dialog-theme>.ui` and `.css` from mate-screensaver's data directory, with a vendored `lock-dialog-default.ui` in a gresource when the file is missing. It has the user's face and real name, the prompt and PAM's messages, and the clock and date in `lock-dialog-time-format` and `lock-dialog-date-format`.
- **PAM:** the service `mate-screensaver`, in a thread, as ported. Fingerprint works through that stack.
- **No popups** on a lock surface, so no tooltip or menu. **No layout indicator:** libmatekbd is compiled out.
- **The build:** its own Meson project beside `caminito-gtk-palette`, never built by cargo (15). `caminito-git` installs it and depends on gtk-session-lock, mate-desktop and mate-screensaver ([16](16-packaging.md)). The licence is GPL-2.0-or-later, with mate-screensaver's notices kept.
- **CI** builds it, and the Packaging job has the new dependencies.

## Acceptance criteria

- [ ] It builds without warnings, and `caminito-git` builds, installs it and passes `namcap`.
- [ ] A headless test with the real `caminito-locker` as `locker-command`, where CI has GTK and gtk-session-lock: it takes the lock and makes a lock surface for each output.
- [x] The PAM conversation is tested against a stand-in service where the test can install one, or the ticket says why not.
- [ ] The checklist has an item, tagged M7, and it was run nested and in a Session: the dialog is MATE's with the GTK theme, a wrong password says so and asks again, the right one unlocks, the dialog follows the pointer's monitor, and a change of `lock-dialog-theme` shows at the next lock. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **gtk-session-lock is barely maintained.** If it doesn't build at 16's baseline, vendor it and say so (ADR 0007).
- **The background, the status message and the Switch User and Log Out buttons** are [132](132-caminito-locker-background-and-buttons.md)'s. Until then the windows have a plain background.

## Done (2026-10-09)

- **Where it was built:** an Ubuntu 24.04 container, with GTK 3.24.41, gtk-session-lock 0.2.0 built from `b9ddb27`, mate-desktop 1.26.2 and Linux-PAM 1.5.3. `helpers/caminito-locker` builds there with no warning at `warning_level=2`, alone and as the top-level project's subproject. CI's Arch job is the first build at 16's baseline: the first criterion waits on it, and on the Packaging job for `caminito-git` and `namcap`. gtk-session-lock built as it is, so it isn't vendored.
- **The tests:**
  - `helpers/caminito-locker/test-auth-pam.c`, a Meson test, runs the ported conversation under pam_wrapper against a stand-in `mate-screensaver` service of pam_matrix and pam_chatty: the right password, a wrong one (*Incorrect password.*), a wrong one then the right one, a cancelled prompt, and a message of the service's reaching the dialog. It's defined only where pam_wrapper is installed; CI's check job installs it.
  - `caminito/src/tests/caminito_locker.rs` runs the real `caminito-locker` against the headless fixture, ignored unless `CAMINITO_LOCKER` names it. On two outputs it takes the lock and the dump has a lock surface on each (`lock_surface`, new in each output's entry) and `locked`. With the stand-in service from `PAM_WRAPPER_MODULES`, a wrong password typed through Caminito's keyboard is refused and asked again, and the right one unlocks and the locker exits 0. Both passed six runs in a row here, and the first failed with the locker's window on the second monitor taken out. CI runs both after the Meson build, which the second criterion waits on.
  - Not as `locker-command`: nothing spawns a locker until [126](126-the-locker-command.md), so the test starts it as a locker started by hand, on a socket of its own. The second criterion's *as `locker-command`* moves to 126, whose spawn test can point at the same binary; the rest of it waits on CI here.
- **Not tested headless:** the dialog following the pointer between monitors, and a monitor plugged in or out while locked. The checklist item's step 5 covers the first; the second is left to 136's run.
- **The checklist item** is *caminito-locker shows MATE's unlock dialog, and the password unlocks*, tagged M7. It wasn't run: the container has no display. It's left to [136](136-packaging-and-closing-m7.md).
- **What the port changed**, each said in the file's header:
  - `GSLockPlug` is a `GtkBin` in one of the lock windows. It moves to the window the pointer enters or moves in, or that gets the keyboard, which Caminito gives the lock surface on the pointer's output. The default button, the keys held back while the prompt isn't ready, and the busy cursor go to whichever window it's in.
  - The dialog built in code when the theme's `.ui` is missing is gone: the vendored `lock-dialog-default.ui` stands in for it, from the gresource, and for a theme whose `.ui` doesn't load.
  - mate-screensaver's default `.ui` has a tooltip, on the entry's eye icon, so the ticket's *the default `.ui` has none* was wrong. The vendored copy drops it, and the locker clears every tooltip of whichever theme it loads.
  - The note and its tab need libnotify, and are hidden as in a mate-screensaver built without it. libmatekbd is out, and the layout indicator's placeholder hidden.
  - PAM waits for its thread in a `GMainLoop` rather than `gtk_main`, so `gs-auth-pam.c` needs no GTK and its test no display. Two bugs of the original are fixed there: the watch's source was removed twice, a GLib critical, and the pipe's read end was never closed.
  - mate-screensaver's dialog ends after the fifth failure, or at a cancel or the 60 s timeout, and its daemon starts another. The locker is the lock and never ends but at the unlock, so each starts the conversation again: a cancel or the timeout at once, a failure once its message has shown for mate-screensaver's 3 s. mate-screensaver-dialog.c starts the next round at once and clears the message 3 s later, which could clear the next round's; waiting also keeps a service that denies everyone from going round at once.
  - From the review: the layout indicator's placeholder is no-show-all, so a lock window's show_all can't show it again, and the entry's menu is held back from the Menu key and Shift+F10 too, not only the right button.
  - The unlock waits for `locked`, as ext-session-lock has `unlock_and_destroy` only after it, in case the password is typed first. A refused lock (`finished`) exits 1.
  - The strings are mate-screensaver's, translated by its catalogue: the locker binds the `mate-screensaver` text domain.
  - The Switch User and Log Out buttons' code and the status message's property are ported, but nothing sets them: they're [132](132-caminito-locker-background-and-buttons.md)'s, with the background.
- **The test schemas** have mate-screensaver's `org.mate.screensaver`, from `60b2db3`, which the locker's tests run on.
