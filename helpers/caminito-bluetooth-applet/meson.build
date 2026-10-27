# The Bluetooth applet (259, ADR 0035): an in-process mate-panel applet in C and GTK3,
# which reads BlueZ over GDBus. Its own project, which the top-level meson.build includes
# as a subproject and installs. It's called "Bluetooth" in Add to Panel.
project(
  'caminito-bluetooth-applet',
  'c',
  version: '0.1.0',
  license: 'GPL-3.0-or-later',
  meson_version: '>= 1.0',
  default_options: ['warning_level=2', 'c_std=gnu11'],
)

gio = dependency('gio-2.0')
applet_dep = dependency('libmatepanelapplet-4.0')
# The popup and the icon, which the Bluetooth, Power and Thermal applets share (ADR 0035).
# A subproject of a subproject is looked for in the top-level project's helpers/.
common = subproject('caminito-applet-common').get_variable('caminito_applet_common_dep')

# What the applet shows, from what BlueZ has, apart from D-Bus so that the tests can
# build any state.
state = static_library('bt-state', 'bt-state.c', dependencies: gio)
bluez = static_library('bt-bluez', 'bt-bluez.c', link_with: state, dependencies: gio)
rfkill = static_library('bt-rfkill', 'bt-rfkill.c', dependencies: gio)
# The popup's content (260), which draws what bt-state lists in the shared popup window.
popup = static_library(
  'bt-popup',
  'bt-popup.c',
  link_with: state,
  dependencies: [gio, common],
  pic: true,
)

applet_dir = get_option('libdir') / 'mate-panel'
shared_module(
  'caminito-bluetooth-applet',
  'bluetooth-applet.c',
  link_with: [state, bluez, rfkill, popup],
  dependencies: [gio, applet_dep, common],
  gnu_symbol_visibility: 'hidden',
  install: true,
  install_dir: applet_dir,
)

# Its entry in Add to Panel, in the normal directory, not Caminito's own (the spec's "Any
# MATE desktop"): the applet works under X11 MATE too.
configure_file(
  input: 'org.mate.panel.CaminitoBluetoothApplet.mate-panel-applet.in',
  output: 'org.mate.panel.CaminitoBluetoothApplet.mate-panel-applet',
  configuration: {
    'APPLET_LOCATION': get_option('prefix') / applet_dir / 'libcaminito-bluetooth-applet.so',
  },
  install: true,
  install_dir: get_option('datadir') / 'mate-panel' / 'applets',
)

# The icon's state and the tooltip, from snapshots built by hand.
test('state', executable('test-state', 'test-state.c', link_with: state, dependencies: gio))
# The popup's content, which needs a display and is skipped without one.
test(
  'popup',
  executable('test-popup', 'test-popup.c', link_with: [popup, state], dependencies: [gio, common]),
  env: {'NO_AT_BRIDGE': '1'},
)
# The rfkill event, written to a file in /dev/rfkill's place.
test('rfkill', executable('test-rfkill', 'test-rfkill.c', link_with: rfkill, dependencies: gio))

# BlueZ against python-dbusmock's bluez5 template, where python-dbusmock is installed.
python = import('python').find_installation(
  'python3',
  modules: ['dbusmock'],
  required: false,
)
if python.found()
  test(
    'bluez',
    executable('test-bluez', 'test-bluez.c', link_with: bluez, dependencies: gio),
    args: [python.full_path(), meson.current_source_dir() / 'bluez-mock.py'],
    timeout: 120,
  )
endif
