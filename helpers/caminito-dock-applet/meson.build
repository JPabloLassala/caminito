# The Dock applet (CMN-219): an in-process mate-panel applet in C and GTK3 (ADR 0035), whose
# Dock items are the desktop entries with windows, read through libxfce4windowing, the
# library mate-panel's own window list reads Caminito's windows with (ADR 0013). Its own
# project, which the top-level meson.build includes as a subproject and installs. It's called
# "Dock" in Add to Panel.
project(
  'caminito-dock-applet',
  'c',
  version: '0.1.0',
  license: 'GPL-3.0-or-later',
  meson_version: '>= 1.0',
  default_options: ['warning_level=2', 'c_std=gnu11'],
)

glib = dependency('glib-2.0')
gio = dependency('gio-2.0')
gio_unix = dependency('gio-unix-2.0')
gtk = dependency('gtk+-3.0')
# The pulse and the bounce are cosines and sines.
m = meson.get_compiler('c').find_library('m')
# The windows, as mate-panel's own window list reads them on Caminito (ADR 0013).
xfw = dependency('libxfce4windowing-0')
applet_dep = dependency('libmatepanelapplet-4.0')
# The applet's shared plumbing (ADR 0035). A subproject of a subproject is looked for in the
# top-level project's helpers/.
common = subproject('caminito-applet-common').get_variable('caminito_applet_common_dep')

# Which desktop entry an app id names.
resolve = static_library('dk-resolve', 'dk-resolve.c', dependencies: [glib, gio_unix], pic: true)

test(
  'resolve',
  executable('test-resolve', 'test-resolve.c', link_with: resolve, dependencies: glib),
)

# The Dock's items from the windows and the kept items, and what a click does.
model = static_library('dk-model', 'dk-model.c', link_with: resolve, dependencies: glib, pic: true)

test(
  'model',
  executable('test-model', 'test-model.c', link_with: [model, resolve], dependencies: glib),
)

# What pulses because it was launched and what bounces and glows because a window wants
# attention (CMN-221), with the clock passed in.
feedback = static_library('dk-feedback', 'dk-feedback.c', link_with: model, dependencies: glib,
                          pic: true)

test(
  'feedback',
  executable('test-feedback', 'test-feedback.c', link_with: [feedback, model, resolve],
             dependencies: glib),
)

# What programs tell their Dock items through Unity's LauncherEntry signal (CMN-227): a count,
# a progress and urgency, kept for each desktop entry.
launcher = static_library('dk-launcher', 'dk-launcher.c', link_with: [model, resolve],
                          dependencies: [glib, m], pic: true)

test(
  'launcher',
  executable('test-launcher', 'test-launcher.c', link_with: [launcher, model, resolve],
             dependencies: [glib, m]),
)

# The kept items and the Kept folders in the applet's relocatable settings. The package's hook compiles the
# schemas, so nothing is compiled for the install.
install_data(
  'org.mate.panel.applet.caminito-dock.gschema.xml',
  install_dir: get_option('datadir') / 'glib-2.0' / 'schemas',
)

# The schema as glib-compile-schemas reads it, strictly.
glib_compile_schemas = find_program('glib-compile-schemas', required: false)
if glib_compile_schemas.found()
  test('schema', glib_compile_schemas,
       args: ['--strict', '--dry-run', meson.current_source_dir()])
  # The schema compiled for the store's test.
  schemas = custom_target(
    'schemas',
    input: 'org.mate.panel.applet.caminito-dock.gschema.xml',
    output: 'gschemas.compiled',
    command: [glib_compile_schemas, '--targetdir', '@OUTDIR@', meson.current_source_dir()],
  )
endif

store = static_library('dk-store', 'dk-store.c', link_with: model, dependencies: [glib, gio],
                       pic: true)

# Keeping, removing and reordering across a restart, over a keyfile backend.
if glib_compile_schemas.found()
  test(
    'store',
    executable('test-store', 'test-store.c', link_with: [store, model, resolve],
               dependencies: [glib, gio]),
    env: {'GSETTINGS_SCHEMA_DIR': meson.current_build_dir()},
    depends: schemas,
  )
endif

# The xdg-activation token of a launch (CMN-221). GDK 3 makes one only for an entry with
# StartupNotify=true, so the dock binds the protocol on GDK's connection and asks for it. The
# protocol's code is generated from its vendored XML.
wayland_client = dependency('wayland-client')
wayland_scanner = find_program('wayland-scanner')
activation_xml = files('protocols/xdg-activation-v1.xml')
activation_header = custom_target(
  'xdg-activation-header',
  input: activation_xml,
  output: 'xdg-activation-v1-client-protocol.h',
  command: [wayland_scanner, 'client-header', '@INPUT@', '@OUTPUT@'],
)
activation_code = custom_target(
  'xdg-activation-code',
  input: activation_xml,
  output: 'xdg-activation-v1-protocol.c',
  command: [wayland_scanner, 'private-code', '@INPUT@', '@OUTPUT@'],
)
activation = static_library(
  'dk-activation',
  'dk-activation.c',
  activation_header,
  activation_code,
  dependencies: [glib, gtk, wayland_client],
  pic: true,
)

# A Kept folder's folder (CMN-239): its icon, its contents and putting files in it, through GIO.
folder = static_library('dk-folder', 'dk-folder.c', dependencies: [glib, gio], pic: true)

test(
  'folder',
  executable('test-folder', 'test-folder.c', link_with: folder, dependencies: [glib, gio]),
)

# The Trash: whether it is full, putting files in it and emptying it, through GIO.
trash = static_library('dk-trash', 'dk-trash.c', dependencies: [glib, gio], pic: true)

test(
  'trash',
  executable('test-trash', 'test-trash.c', link_with: trash, dependencies: [glib, gio]),
)

# The dock: the items' widgets, the windows, the clicks, the menu and the drags.
dock = static_library(
  'dk-dock',
  'dk-dock.c',
  'dk-item.c',
  link_with: [activation, feedback, launcher, store, model, resolve, trash, folder],
  dependencies: [glib, gio_unix, gtk, xfw, m],
  pic: true,
)

# The probe is a layer surface where the compositor has the layer shell, as the panel is.
layer_shell = dependency('gtk-layer-shell-0', required: false)
probe_args = layer_shell.found() ? ['-DHAVE_GTK_LAYER_SHELL'] : []

# The dock in a window of its own: caminito's dock_applet test runs it under a headless
# Caminito (CAMINITO_DOCK_PROBE names it), and the check script shows it. Not installed.
executable(
  'dk-probe',
  'dk-probe.c',
  link_with: [dock, activation, feedback, launcher, store, model, resolve, trash, folder],
  # The layer shell first: it has to be linked before libwayland-client.
  dependencies: [layer_shell, glib, gio_unix, gtk, xfw, wayland_client, m],
  c_args: probe_args,
)

applet_dir = get_option('libdir') / 'mate-panel'
shared_module(
  'caminito-dock-applet',
  'dock-applet.c',
  link_with: [dock, activation, feedback, launcher, store, model, resolve, trash, folder],
  dependencies: [glib, gio_unix, gtk, xfw, wayland_client, m, applet_dep, common],
  gnu_symbol_visibility: 'hidden',
  install: true,
  install_dir: applet_dir,
)

# Its entry in Add to Panel, in the normal directory, not Caminito's own: the applet works
# under X11 MATE too.
configure_file(
  input: 'org.mate.panel.CaminitoDockApplet.mate-panel-applet.in',
  output: 'org.mate.panel.CaminitoDockApplet.mate-panel-applet',
  configuration: {
    'APPLET_LOCATION': get_option('prefix') / applet_dir / 'libcaminito-dock-applet.so',
  },
  install: true,
  install_dir: get_option('datadir') / 'mate-panel' / 'applets',
)
