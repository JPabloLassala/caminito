# Camino's C helpers and data files (16). Cargo builds the compositor itself, driven by
# the PKGBUILD: Meson never builds Rust, and cargo never builds C (15).
#
# Nothing is built or installed yet. M2 adds the camino-gtk-palette helper and M5 the
# session files, each helper as its own Meson project under helpers/.
project(
  'camino',
  license: 'GPL-2.0-or-later',
  meson_version: '>=1.1',
  subproject_dir: 'helpers',
)
