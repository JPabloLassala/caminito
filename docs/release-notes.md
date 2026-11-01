# Release notes

What someone upgrading has to do by hand.

## The Lock screen without the mate-screensaver package (CMN-297, ADR 0056)

- **A customised PAM file has to be redone.** caminito-locker now authenticates through
  `/etc/pam.d/caminito-locker`, and no longer reads `/etc/pam.d/mate-screensaver`. If you changed
  that file, for fingerprint for example, make the same change in the new one. If the new file is
  missing, the Locker exits and the session stays locked, with a message that names
  `loginctl unlock-session`. A service only in `/etc/pam.conf` doesn't count.
- **A dialog theme has to be copied.** `lock-dialog-theme` is read from `/usr/share/caminito/`
  alone: copy a theme's `lock-dialog-<name>.ui` and `.css` there. Without them the dialog is
  the built-in one. Delete `auth-note-button` from a copied `.ui`: the note is gone, and the button
  would show and do nothing.
- **The package can go.** `mate-screensaver` is no longer a dependency, and can be removed unless
  an X11 session uses it. Removing it also removes `/usr/share/backgrounds/cosmos/`, whose dynamic
  wallpapers then leave the Wallpaper dialog. With it installed, `glib-compile-schemas` says it ignores one of the two
  copies of `org.mate.screensaver`, which is harmless.
- **Switch User** only asks a display manager over D-Bus, as LightDM and sddm answer. It hides
  unless the seat in `XDG_SEAT_PATH` answers that it can switch, so it no longer shows under GDM
  or MDM.
