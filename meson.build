# Camino's C helpers and data files (16). Cargo builds the compositor itself, driven by
# the PKGBUILD: Meson never builds Rust, and cargo never builds C (15).
#
# Each helper is its own Meson project under helpers/. C is declared here although this
# project builds nothing itself: Meson reads CFLAGS and LDFLAGS, makepkg's hardening flags
# among them, only for the top-level project's languages.
project(
  'camino',
  'c',
  license: 'GPL-2.0-or-later',
  meson_version: '>=1.1',
  subproject_dir: 'helpers',
)

subproject('camino-gtk-palette')

# The display manager's entry for the session (08). Its name isn't the Wayfire session's
# MATE.desktop, and it needs no wrapper script and no systemd unit.
install_data(
  'data/mate-camino.desktop',
  install_dir: get_option('datadir') / 'wayland-sessions',
)

# camino-displays's entry, in the Display capplet's place (84). It has the capplet's id,
# mate-display-properties.desktop, in a directory that only a Session's programs look in,
# before the stock packages': Camino puts it first in its session command's XDG_DATA_DIRS.
# So the control center and the menus show camino-displays where they showed the capplet,
# and X11 MATE and the Wayfire session still find the capplet's own entry.
install_data(
  'data/camino-displays.desktop',
  rename: 'mate-display-properties.desktop',
  install_dir: get_option('datadir') / 'camino' / 'xdg' / 'applications',
)

# What the control center's Keyboard Shortcuts dialog lists under Camino (05, 101): Marco's
# own manifests from marco 1.29.0, with Camino's name in wm_name, which the dialog matches
# against MATE_WM_KEYBINDINGS. They point at the org.mate.Marco.* keys, and their names are
# translated from marco's catalogue. No action Camino never implements (05) was in Marco's,
# so none is left out. The directory is mate-control-center's, where marco installs its own
# too.
install_data(
  'data/keybindings/50-camino-desktop-key.xml',
  'data/keybindings/50-camino-global-key.xml',
  'data/keybindings/50-camino-window-key.xml',
  install_dir: get_option('datadir') / 'mate-control-center' / 'keybindings',
)
