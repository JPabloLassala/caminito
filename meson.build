# Camino's C helpers and data files (16). Cargo builds the compositor itself, driven by
# the PKGBUILD: Meson never builds Rust, and cargo never builds C (15).
#
# Each helper is its own Meson project under helpers/. M5 adds the session files. C is
# declared here although this project builds nothing itself: Meson reads CFLAGS and LDFLAGS,
# makepkg's hardening flags among them, only for the top-level project's languages.
project(
  'camino',
  'c',
  license: 'GPL-2.0-or-later',
  meson_version: '>=1.1',
  subproject_dir: 'helpers',
)

subproject('camino-gtk-palette')
