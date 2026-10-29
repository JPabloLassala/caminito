# The GTK colours for Caminito's frames (07, ADR 0003). Its own project (15), which the
# top-level meson.build includes as a subproject and installs.
project(
  'caminito-gtk-palette',
  'c',
  version: '0.1.0',
  license: 'GPL-3.0-or-later',
  meson_version: '>= 1.0',
  default_options: ['warning_level=2', 'c_std=gnu11'],
)

executable(
  'caminito-gtk-palette',
  'caminito-gtk-palette.c',
  dependencies: [dependency('gtk+-3.0'), dependency('gio-unix-2.0')],
  install: true,
)
