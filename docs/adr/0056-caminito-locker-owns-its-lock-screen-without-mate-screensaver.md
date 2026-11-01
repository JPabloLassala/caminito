# 56. caminito-locker owns its Lock screen, without the mate-screensaver package

Date: 2026-10-31
Status: Accepted

## Context

ADR 0007 ported mate-screensaver's unlock dialog and kept the package installed for its dialog theme, its PAM service and its translations. That installs an X11 daemon which Caminito then has to keep from running. The port also dropped the daemon that started the dialog at a key and ended it at a timeout, but kept the timeout. The dialog now shows for the whole lock and cancels a PAM conversation each minute, which `pam_faillock` counts as a failure (CMN-256). The developer also wants features mate-screensaver never had, such as notifications on the Lock screen.

## Decision

- **caminito-locker stays C and GTK3 on gtk-session-lock, with a dialog derived from MATE's.** The core of ADR 0007 stands: Caminito locks first, the Locker only draws, and `locker-command` replaces it.
- **The `mate-screensaver` package is no longer a dependency.** Caminito ships a `caminito-locker` PAM service for each distribution, its own dialog files, and a `caminito-locker` translation domain with the strings it uses, copied from mate-screensaver's. `lock-dialog-theme` is still honoured, from Caminito's data directory only.
- **The Lock screen has two states.** The Idle view is the wallpaper, a clock and any notifications its mode allows. The Unlock dialog shows at a key or a click, but not at pointer motion, and hides after 60 s. Only a submitted password ends a PAM conversation, which stays open while the dialog is hidden, so a fingerprint can unlock from the Idle view.
- **A missing PAM service is a failed start.** The Locker exits rather than show a prompt that can't succeed, and Caminito's give-up message names `loginctl unlock-session`. It never falls back to the `mate-screensaver` service.
- **The Locker asks for Esmeril behind the Unlock dialog, and never blurs anything itself.** Its sharp wallpaper is in a subsurface under its see-through window, which asks through `ext-background-effect-v1` (CMN-238). It fades by lowering the opacity of a sharp copy in that window. Esmeril goes between a lock surface's own layers, so it reads only the Locker's wallpaper. A spike has yet to show that this can be done.

## Consequences

- A user who customised the `mate-screensaver` PAM file, for fingerprint for example, has to redo it in the new file. The release notes say so.
- Themes written for mate-screensaver have to be copied to Caminito's data directory.
- The package also had the `org.mate.screensaver` schema and `mate-screensaver-command`, which the Locker, the panel, mate-session and the media keys need. Caminito ships a copy of the schema under its own filename, so that the two packages don't conflict: `glib-compile-schemas` ignores one with an error and exits 0, unless run with `--strict`. It ships the command in `/usr/lib/caminito/bin`, first in the session command's `PATH`, and the command only calls `org.mate.ScreenSaver`.
- `mate-screensaver.desktop` stays in mate-session's skipped list (ADR 0014), as the package can still be installed for an X11 session.
- Pointer motion doesn't wake the dialog, unlike mate-screensaver. `docs/parity.md` records this.
- Rejected:
  - keeping the dependency, which is cheaper in translations but leaves an X11 daemon installed for its data;
  - falling back to the `mate-screensaver` service, which would make the stack that guards the session depend on what else is installed;
  - a rewrite in Rust and GTK4, which isn't justified while the Lock screen's new features sit beside a dialog that works;
  - a CPU blur in the Locker, which is simple but is a second blur to keep looking like Esmeril, as ADR 0048 rejected for Pixman;
  - Caminito drawing the wallpaper and Esmeril under a see-through Locker, which has the lock scene draw another program's window and loses `picture-filename`.
