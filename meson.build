# Caminito's C helpers and data files (16). Cargo builds the compositor itself, driven by
# the PKGBUILD: Meson never builds Rust, and cargo never builds C (15).
#
# Each helper is its own Meson project under helpers/. C is declared here although this
# project builds nothing itself: Meson reads CFLAGS and LDFLAGS, makepkg's hardening flags
# among them, only for the top-level project's languages.
project(
  'caminito',
  'c',
  license: 'GPL-2.0-or-later',
  meson_version: '>=1.1',
  subproject_dir: 'helpers',
)

subproject('caminito-gtk-palette')
subproject('caminito-locker')
# The Bluetooth applet (259), which pulls in caminito-applet-common (ADR 0035).
subproject('caminito-bluetooth-applet')

# The display manager's entry for the session (08). Its name isn't the Wayfire session's
# MATE.desktop, and it needs no wrapper script and no systemd unit.
install_data(
  'data/mate-caminito.desktop',
  install_dir: get_option('datadir') / 'wayland-sessions',
)

# The logo (109), the session entry's icon: Icon=caminito finds it in the hicolor theme,
# which every icon theme falls back to.
install_data(
  'data/icons/caminito.svg',
  install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'scalable' / 'apps',
)

# Caminito's own settings (09): the workspace grid's three keys (96). The package's hook
# compiles the schemas (16), so nothing is compiled here.
install_data(
  'data/org.mate.Caminito.gschema.xml',
  install_dir: get_option('datadir') / 'glib-2.0' / 'schemas',
)

# caminito-displays's entry, in the Display capplet's place (84). It has the capplet's id,
# mate-display-properties.desktop, in a directory that only a Session's programs look in,
# before the stock packages': Caminito puts it first in its session command's XDG_DATA_DIRS.
# So the control center and the menus show caminito-displays where they showed the capplet,
# and X11 MATE and the Wayfire session still find the capplet's own entry.
install_data(
  'data/caminito-displays.desktop',
  rename: 'mate-display-properties.desktop',
  install_dir: get_option('datadir') / 'caminito' / 'xdg' / 'applications',
)

# caminito-about's entry (182), in the same directory, so that only a Session lists it:
# under X11 MATE it would describe a compositor that isn't running. Its categories put it
# under Hardware in the control center, beside System Info and Displays.
install_data(
  'data/caminito-about.desktop',
  install_dir: get_option('datadir') / 'caminito' / 'xdg' / 'applications',
)

# The Wallpaper dialog's entry (212), in the same directory, so that only a Session lists it:
# under X11 MATE msd's background plugin draws, and the dialog would set what nothing draws.
# Its categories are Appearance's, which put it under Look and Feel in the control center.
install_data(
  'data/caminito-wallpaper.desktop',
  install_dir: get_option('datadir') / 'caminito' / 'xdg' / 'applications',
)

# caminito-bluetooth's entries (255). Unlike the entries above, these are in the normal XDG
# directories: it doesn't depend on Caminito, and works under X11 MATE too (ADR 0035). Its
# categories put the window under Hardware in the control center. The autostart entry starts
# it at login with --background, for the pairing agent (256).
install_data(
  'data/caminito-bluetooth.desktop',
  install_dir: get_option('datadir') / 'applications',
)
install_data(
  'data/caminito-bluetooth-autostart.desktop',
  rename: 'caminito-bluetooth.desktop',
  install_dir: get_option('sysconfdir') / 'xdg' / 'autostart',
)

# The default panel (CMN-105): mate-panel's default layout with the Bluetooth applet beside the
# notification area, and the override that makes it the default. Like caminito-bluetooth's
# entries these are in the normal XDG directories, not share/caminito/xdg/: the layout is
# right under X11 MATE too, and the panel finds it by name in its own layouts directory.
# Only a user with no panel settings gets it (the applet is in Add to Panel for the rest).
# The package's hook compiles the override with the schemas.
install_data(
  'data/caminito.layout',
  install_dir: get_option('datadir') / 'mate-panel' / 'layouts',
)
install_data(
  'data/90_caminito.gschema.override',
  install_dir: get_option('datadir') / 'glib-2.0' / 'schemas',
)

# The launcher of the control center's Keyboard Shortcuts dialog (183), under the stock entry's
# id and in the same directory, so that it shadows it in a Session alone. There the dialog is
# System Shortcuts, and the patched dialog takes its title from this entry's name, so the
# Spanish is carried here. Its comment and keywords are the stock entry's, from
# mate-control-center 1.29.1, in every language that has them: only the name changes, and
# a language other than Spanish shows the English one. X11 MATE and the Wayfire session still
# find the stock entry.
install_data(
  'data/mate-keybinding.desktop',
  install_dir: get_option('datadir') / 'caminito' / 'xdg' / 'applications',
)

# What the control center's System Shortcuts dialog lists under Caminito (05, 101): Marco's
# own manifests from marco 1.29.0, with Caminito's name in wm_name, which the dialog matches
# against MATE_WM_KEYBINDINGS. They point at the org.mate.Marco.* keys, and their names are
# translated from marco's catalogue. No action Caminito never implements (05) was in Marco's,
# so none is left out. The Overview's (241) is Caminito's own, and points at its key in
# org.mate.Caminito: marco's catalogue hasn't its name, which shows in English. The
# directory is mate-control-center's, where marco installs its own too.
install_data(
  'data/keybindings/50-caminito-desktop-key.xml',
  'data/keybindings/50-caminito-global-key.xml',
  'data/keybindings/50-caminito-overview-key.xml',
  'data/keybindings/50-caminito-window-key.xml',
  install_dir: get_option('datadir') / 'mate-control-center' / 'keybindings',
)
