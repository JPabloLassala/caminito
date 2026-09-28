# Camino's C helpers and data files (16). Cargo builds the compositor itself, driven by
# the PKGBUILD: Meson never builds Rust, and cargo never builds C (15).
#
# Each helper is its own Meson project under helpers/. M5 adds the session files.
project(
  'camino',
  license: 'GPL-2.0-or-later',
  meson_version: '>=1.1',
  subproject_dir: 'helpers',
)

subproject('camino-gtk-palette')
