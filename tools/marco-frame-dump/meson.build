# The reference side of the theme goldens (14, ADR 0009). Its own project: Camino's
# top-level meson.build doesn't include it and nothing packages it. tools/regenerate-goldens
# builds and runs it.
project(
  'marco-frame-dump',
  'c',
  version: '0',
  meson_version: '>= 1.0',
  default_options: ['warning_level=2', 'c_std=gnu11'],
)

executable(
  'marco-frame-dump',
  'marco-frame-dump.c',
  dependencies: [
    dependency('libmarco-private'),
    dependency('gtk+-3.0'),
    dependency('json-glib-1.0'),
  ],
  install: false,
)
