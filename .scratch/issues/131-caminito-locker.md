# caminito-locker: MATE's unlock dialog on a lock surface

Epic: caminito-v0.1
Type: implementation
Status: open
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
- [ ] The PAM conversation is tested against a stand-in service where the test can install one, or the ticket says why not.
- [ ] The checklist has an item, tagged M7, and it was run nested and in a Session: the dialog is MATE's with the GTK theme, a wrong password says so and asks again, the right one unlocks, the dialog follows the pointer's monitor, and a change of `lock-dialog-theme` shows at the next lock. If it can't be run where the ticket is done, say so in the notes and leave it to [136](136-packaging-and-closing-m7.md).

## Notes

- **gtk-session-lock is barely maintained.** If it doesn't build at 16's baseline, vendor it and say so (ADR 0007).
- **The background, the status message and the Switch User and Log Out buttons** are [132](132-caminito-locker-background-and-buttons.md)'s. Until then the windows have a plain background.
