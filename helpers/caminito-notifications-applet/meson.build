# The Notifications applet (CMN-260, ADR 0051): an in-process mate-panel applet in C and GTK3,
# the bell and the Notification center, which reads caminito-notifications's private API over
# GDBus. Its own project, which the top-level meson.build includes as a subproject and
# installs. It's called "Notifications" in Add to Panel.
project(
  'caminito-notifications-applet',
  'c',
  version: '0.1.0',
  license: 'GPL-3.0-or-later',
  meson_version: '>= 1.0',
  default_options: ['warning_level=2', 'c_std=gnu11'],
)

glib = dependency('glib-2.0')
gio = dependency('gio-2.0')
applet_dep = dependency('libmatepanelapplet-4.0')
# The popup and the icon, which the applets share (ADR 0035). A subproject of a subproject is
# looked for in the top-level project's helpers/.
common = subproject('caminito-applet-common').get_variable('caminito_applet_common_dep')

# The history's entries as List gives them, their age, and the icon and the tooltip, apart
# from D-Bus so that the tests can build any history.
state = static_library('nt-state', 'nt-state.c', dependencies: glib)
# The daemon's private API over GDBus (ADR 0051).
daemon = static_library('nt-daemon', 'nt-daemon.c', link_with: state, dependencies: gio)
# The Notification center, in the shared popup window.
popup = static_library('nt-popup', 'nt-popup.c', link_with: state, dependencies: common, pic: true)

applet_dir = get_option('libdir') / 'mate-panel'
shared_module(
  'caminito-notifications-applet',
  'notifications-applet.c',
  link_with: [state, daemon, popup],
  dependencies: [gio, applet_dep, common],
  gnu_symbol_visibility: 'hidden',
  install: true,
  install_dir: applet_dir,
)

# Its entry in Add to Panel, in the normal directory, not Caminito's own, as the other
# applets'. It hides itself under X11 MATE, where caminito-notifications isn't running.
configure_file(
  input: 'org.mate.panel.CaminitoNotificationsApplet.mate-panel-applet.in',
  output: 'org.mate.panel.CaminitoNotificationsApplet.mate-panel-applet',
  configuration: {
    'APPLET_LOCATION': get_option('prefix') / applet_dir / 'libcaminito-notifications-applet.so',
  },
  install: true,
  install_dir: get_option('datadir') / 'mate-panel' / 'applets',
)

# The bell's last fallbacks, struck through for Do Not Disturb too, in hicolor, which every icon theme falls back to: MATE's themes
# have no bell, and Adwaita's has the name of MATE's speech bubble.
install_data(
  'caminito-bell-symbolic.svg',
  'caminito-bell-disabled-symbolic.svg',
  install_dir: get_option('datadir') / 'icons' / 'hicolor' / 'scalable' / 'status',
)

# The entries, ages and tooltip, from List replies built by hand.
test('state', executable('test-state', 'test-state.c', link_with: state, dependencies: glib))

# The Notification center, which needs a display and is skipped without one.
test(
  'popup',
  executable('test-popup', 'test-popup.c', link_with: [popup, state], dependencies: common),
  # UTC, so that the times Do Not Disturb ends at read the same everywhere.
  env: {'NO_AT_BRIDGE': '1', 'TZ': 'UTC'},
)

# The private API against a mock of it, on a private session bus, where python-dbusmock is
# installed.
python = import('python').find_installation(
  'python3',
  modules: ['dbusmock'],
  required: false,
)
if python.found()
  test(
    'daemon',
    executable('test-daemon', 'test-daemon.c', link_with: [daemon, state], dependencies: gio),
    args: [python.full_path(), meson.current_source_dir() / 'notifications-mock.py'],
    timeout: 120,
  )
endif
