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

# Camino's own settings (09): the workspace grid's three keys (96). The package's hook
# compiles the schemas (16), so nothing is compiled here.
install_data(
  'data/org.mate.Camino.gschema.xml',
  install_dir: get_option('datadir') / 'glib-2.0' / 'schemas',
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
