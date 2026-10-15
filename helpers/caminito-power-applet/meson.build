# The Power applet (262, ADR 0035): an in-process mate-panel applet in C and GTK3, which
# reads UPower over GDBus. Its own project, which the top-level meson.build includes as a
# subproject and installs. It's called "Power" in Add to Panel.
project(
  'caminito-power-applet',
  'c',
  version: '0.1.0',
  license: 'GPL-2.0-or-later',
  meson_version: '>= 1.0',
  default_options: ['warning_level=2', 'c_std=gnu11'],
)

gio = dependency('gio-2.0')
# The popup names an application by its desktop entry.
gio_unix = dependency('gio-unix-2.0')
m = meson.get_compiler('c').find_library('m')
applet_dep = dependency('libmatepanelapplet-4.0')
# The popup and the icon, which the Bluetooth, Power and Thermal applets share (ADR 0035).
# A subproject of a subproject is looked for in the top-level project's helpers/.
common = subproject('caminito-applet-common').get_variable('caminito_applet_common_dep')

# What the applet shows, from what UPower has, apart from D-Bus so that the tests can
# build any state.
state = static_library('pw-state', 'pw-state.c', dependencies: [gio, m])
upower = static_library('pw-upower', 'pw-upower.c', link_with: state, dependencies: gio)
# power-profiles-daemon's profiles and what's said of them, and the daemon itself.
profiles = static_library('pw-profiles', 'pw-profiles.c', dependencies: gio)
ppd = static_library('pw-ppd', 'pw-ppd.c', link_with: profiles, dependencies: gio, pic: true)
popup = static_library(
  'pw-popup',
  ['pw-popup.c', 'pw-profiles-section.c'],
  link_with: [ppd, profiles],
  dependencies: [gio, gio_unix, common],
  pic: true,
)

applet_dir = get_option('libdir') / 'mate-panel'
shared_module(
  'caminito-power-applet',
  'power-applet.c',
  link_with: [state, upower, popup],
  dependencies: [gio, applet_dep, common],
  gnu_symbol_visibility: 'hidden',
  install: true,
  install_dir: applet_dir,
)

# Its entry in Add to Panel, in the normal directory, not Caminito's own: the applet works
# under X11 MATE too.
configure_file(
  input: 'org.mate.panel.CaminitoPowerApplet.mate-panel-applet.in',
  output: 'org.mate.panel.CaminitoPowerApplet.mate-panel-applet',
  configuration: {
    'APPLET_LOCATION': get_option('prefix') / applet_dir / 'libcaminito-power-applet.so',
  },
  install: true,
  install_dir: get_option('datadir') / 'mate-panel' / 'applets',
)

# Its preference, "Show percentage". The package's hook compiles the schemas, so nothing is
# compiled here.
install_data(
  'org.mate.panel.applet.caminito-power.gschema.xml',
  install_dir: get_option('datadir') / 'glib-2.0' / 'schemas',
)

# The icon's names and the tooltip, from displays built by hand.
test('state', executable('test-state', 'test-state.c', link_with: state, dependencies: [gio, m]))

# The profiles' text and parsing, from variants built by hand.
test(
  'profiles',
  executable('test-profiles', 'test-profiles.c', link_with: profiles, dependencies: gio),
)

# UPower against python-dbusmock's upower template, where python-dbusmock is installed.
python = import('python').find_installation(
  'python3',
  modules: ['dbusmock'],
  required: false,
)
if python.found()
  test(
    'upower',
    executable('test-upower', 'test-upower.c', link_with: [upower, state], dependencies: [gio, m]),
    args: [python.full_path()],
    timeout: 120,
  )
  # power-profiles-daemon against python-dbusmock's two templates, one for each bus name.
  test(
    'ppd',
    executable('test-ppd', 'test-ppd.c', link_with: [ppd, profiles], dependencies: gio),
    args: [python.full_path()],
    timeout: 120,
  )
endif
