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
