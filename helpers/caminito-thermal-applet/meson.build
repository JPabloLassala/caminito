# The Thermal applet (267, ADR 0035): an in-process mate-panel applet in C and GTK3, which
# reads every chip's temperatures and fans through libsensors. Its own project, which the
# top-level meson.build includes as a subproject and installs. It's called "Temperatures and
# Fans" in Add to Panel.
project(
  'caminito-thermal-applet',
  'c',
  version: '0.1.0',
  license: 'GPL-2.0-or-later',
  meson_version: '>= 1.0',
  default_options: ['warning_level=2', 'c_std=gnu11'],
)

cc = meson.get_compiler('c')
glib = dependency('glib-2.0', version: '>= 2.76')
m = cc.find_library('m')
# lm_sensors installs no pkg-config file.
sensors = cc.find_library('sensors', has_headers: ['sensors/sensors.h'])
applet_dep = dependency('libmatepanelapplet-4.0')
# The popup and the icon, which the Bluetooth, Power and Thermal applets share (ADR 0035).
# A subproject of a subproject is looked for in the top-level project's helpers/.
common = subproject('caminito-applet-common').get_variable('caminito_applet_common_dep')

# What the applet shows, from what the source read, apart from libsensors so that the tests
# can build any computer.
model = static_library('th-model', 'th-model.c', dependencies: [glib, m], pic: true)
# The popup's tiles and Show all sensors (268), from the model.
popup = static_library('th-popup', 'th-popup.c', link_with: model, dependencies: [glib, m, common],
                       pic: true)

applet_dir = get_option('libdir') / 'mate-panel'
shared_module(
  'caminito-thermal-applet',
  'thermal-applet.c',
  'th-source.c',
  link_with: [model, popup],
  dependencies: [glib, m, sensors, applet_dep, common],
  c_args: ['-DVERSION="@0@"'.format(meson.project_version())],
  gnu_symbol_visibility: 'hidden',
  install: true,
  install_dir: applet_dir,
)

# Its entry in Add to Panel, in the normal directory, not Caminito's own: the applet works
# under X11 MATE too.
configure_file(
  input: 'org.mate.panel.CaminitoThermalApplet.mate-panel-applet.in',
  output: 'org.mate.panel.CaminitoThermalApplet.mate-panel-applet',
  configuration: {
    'APPLET_LOCATION': get_option('prefix') / applet_dir / 'libcaminito-thermal-applet.so',
  },
  install: true,
  install_dir: get_option('datadir') / 'mate-panel' / 'applets',
)

# The thermometer's last fallback, in hicolor, which every icon theme falls back to: neither
# MATE's themes nor Adwaita have a temperature icon.
install_data(
  'caminito-thermometer-symbolic.svg',
  install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'scalable' / 'status',
)

# The groups, names and bands, what's left out, the panel sensors and the panel's text,
# from computers built by hand.
test(
  'model',
  executable('test-model', 'test-model.c', 'test-computers.c', link_with: model,
             dependencies: [glib, m]),
)

# The popup's tiles, rows, bars and bands (268), which need a display and are skipped
# without one.
test(
  'popup',
  executable('test-popup', 'test-popup.c', 'test-computers.c', link_with: [popup, model],
             dependencies: [glib, m, common]),
  env: {'NO_AT_BRIDGE': '1'},
)

# The source against fake hwmon trees that umockdev makes, where umockdev is installed: drives'
# models, a reading that can't be read, and chips that come and go.
umockdev = dependency('umockdev-1.0', required: false)
umockdev_wrapper = find_program('umockdev-wrapper', required: false)
if umockdev.found() and umockdev_wrapper.found()
  test(
    'source',
    umockdev_wrapper,
    args: executable(
      'test-source',
      'test-source.c',
      'th-source.c',
      link_with: model,
      dependencies: [glib, m, sensors, umockdev],
    ),
  )
endif
