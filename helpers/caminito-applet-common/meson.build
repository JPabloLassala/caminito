# What the Bluetooth, Power and Thermal applets share (ADR 0035): the popup placed against
# the applet, which left-click toggles and a click elsewhere closes, and the panel icon
# themed with fallbacks. A static library, which each applet's in-process module links in;
# each applet's own logic stays its own.
project(
  'caminito-applet-common',
  'c',
  version: '0.1.0',
  license: 'GPL-2.0-or-later',
  meson_version: '>= 1.0',
  default_options: ['warning_level=2', 'c_std=gnu11'],
)

applet_dep = dependency('libmatepanelapplet-4.0')

common = static_library(
  'caminito-applet-common',
  'caminito-applet-icon.c',
  'caminito-applet-popup.c',
  dependencies: applet_dep,
  # Linked into the applets' shared modules.
  pic: true,
)

caminito_applet_common_dep = declare_dependency(
  link_with: common,
  include_directories: include_directories('.'),
  dependencies: applet_dep,
)

# Its clicks need a display, which the test skips without; the placement doesn't.
test(
  'popup',
  executable('test-popup', 'test-popup.c', dependencies: caminito_applet_common_dep),
  env: {'NO_AT_BRIDGE': '1'},
)
