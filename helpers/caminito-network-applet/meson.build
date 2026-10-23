# The Network applet (CMN-193, ADR 0041): an in-process mate-panel applet in C and GTK3, which
# reads NetworkManager through libnm. Its own project, which the top-level meson.build
# includes as a subproject and installs. It's called "Network" in Add to Panel.
project(
  'caminito-network-applet',
  'c',
  version: '0.1.0',
  license: 'GPL-2.0-or-later',
  meson_version: '>= 1.0',
  default_options: ['warning_level=2', 'c_std=gnu11'],
)

glib = dependency('glib-2.0')
gio = dependency('gio-2.0')
libnm = dependency('libnm')
applet_dep = dependency('libmatepanelapplet-4.0')
# The popup and the icon, which the applets share (ADR 0035). A subproject of a subproject is
# looked for in the top-level project's helpers/.
common = subproject('caminito-applet-common').get_variable('caminito_applet_common_dep')

# What the icon shows, from a status built by NetworkManager's reader or by hand.
state = static_library('nw-state', 'nw-state.c', dependencies: glib)

# The icon's names and the tooltip, from statuses built by hand.
test('state', executable('test-state', 'test-state.c', link_with: state, dependencies: glib))

# NetworkManager through libnm (ADR 0041).
nm = static_library('nw-nm', 'nw-nm.c', link_with: state, dependencies: [gio, libnm])
# The popup's content, in the shared popup window.
popup = static_library('nw-popup', 'nw-popup.c', dependencies: common, pic: true)

applet_dir = get_option('libdir') / 'mate-panel'
shared_module(
  'caminito-network-applet',
  'network-applet.c',
  link_with: [state, nm, popup],
  dependencies: [gio, libnm, applet_dep, common],
  gnu_symbol_visibility: 'hidden',
  install: true,
  install_dir: applet_dir,
)

# Its entry in Add to Panel, in the normal directory, not Caminito's own: the applet works
# under X11 MATE too.
configure_file(
  input: 'org.mate.panel.CaminitoNetworkApplet.mate-panel-applet.in',
  output: 'org.mate.panel.CaminitoNetworkApplet.mate-panel-applet',
  configuration: {
    'APPLET_LOCATION': get_option('prefix') / applet_dir / 'libcaminito-network-applet.so',
  },
  install: true,
  install_dir: get_option('datadir') / 'mate-panel' / 'applets',
)

# The popup's content, which needs a display and is skipped without one.
test(
  'popup',
  executable('test-popup', 'test-popup.c', link_with: popup, dependencies: common),
  env: {'NO_AT_BRIDGE': '1'},
)

# NetworkManager against python-dbusmock's networkmanager template, where python-dbusmock is
# installed.
python = import('python').find_installation(
  'python3',
  modules: ['dbusmock'],
  required: false,
)
if python.found()
  test(
    'nm',
    executable('test-nm', 'test-nm.c', link_with: [nm, state], dependencies: [gio, libnm]),
    args: [python.full_path()],
    timeout: 120,
  )
endif
