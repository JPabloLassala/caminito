# The Sound applet (CMN-213): an in-process mate-panel applet in C and GTK3 (ADR 0035), which
# reads the sound server through libpulse with its GLib main loop: PipeWire through
# pipewire-pulse, or PulseAudio. Its own project, which the top-level meson.build includes as
# a subproject and installs. It's called "Sound" in Add to Panel.
project(
  'caminito-sound-applet',
  'c',
  version: '0.1.0',
  license: 'GPL-3.0-or-later',
  meson_version: '>= 1.0',
  default_options: ['warning_level=2', 'c_std=gnu11'],
)

glib = dependency('glib-2.0')
gio = dependency('gio-2.0')
libpulse = dependency('libpulse')
libpulse_glib = dependency('libpulse-mainloop-glib')
libm = meson.get_compiler('c').find_library('m', required: false)
# The tests' event sound, as Caminito plays the bell (ADR 0027).
libcanberra = dependency('libcanberra')
applet_dep = dependency('libmatepanelapplet-4.0')
# The popup and the icon, which the applets share (ADR 0035). A subproject of a subproject is
# looked for in the top-level project's helpers/.
common = subproject('caminito-applet-common').get_variable('caminito_applet_common_dep')

# What the icon shows, from a status built by the sound server's reader or by hand. It needs
# libpulse's headers alone.
state = static_library(
  'sd-state',
  'sd-state.c',
  dependencies: [glib, libpulse.partial_dependency(compile_args: true)],
  pic: true,
)

# The icon's names, the tooltip and the scroll's volume, from statuses built by hand.
test(
  'state',
  executable(
    'test-state',
    'test-state.c',
    link_with: state,
    dependencies: [glib, libpulse.partial_dependency(compile_args: true)],
  ),
)

# The sound server through libpulse.
pulse = static_library(
  'sd-pulse',
  'sd-pulse.c',
  link_with: state,
  # gio for wpctl, which turns WirePlumber's target restore off (ADR 0045).
  dependencies: [glib, gio, libpulse, libpulse_glib],
  pic: true,
)

# The sound server's reader against a private PipeWire, WirePlumber and pipewire-pulse with
# null devices, where they're installed.
test(
  'pulse',
  executable('test-pulse', 'test-pulse.c', link_with: [pulse, state],
             dependencies: [gio, libpulse, libpulse_glib, libcanberra, libm]),
  timeout: 180,
)

# The popup's content, in the shared popup window.
popup = static_library('sd-popup', 'sd-popup.c', link_with: state,
                       dependencies: [common, libpulse.partial_dependency(compile_args: true)],
                       pic: true)

# The popup's rows, lists, wheel, height and keyboard, from statuses built by hand, where
# there's a display. On X11, whose wheel click is the one the test sends.
test(
  'popup',
  executable('test-popup', 'test-popup.c', link_with: [popup, state],
             dependencies: [common, libpulse.partial_dependency(compile_args: true)]),
  env: {'NO_AT_BRIDGE': '1', 'GDK_BACKEND': 'x11'},
)

# The popup's wheel end to end: caminito's sound_popup_wheel test runs this under a headless
# Caminito, on GDK's Wayland backend (CAMINITO_SOUND_WHEEL_PROBE names it). Not installed.
executable('sd-wheel-probe', 'sd-wheel-probe.c', link_with: [popup, state],
           dependencies: [common, libpulse.partial_dependency(compile_args: true)])

applet_dir = get_option('libdir') / 'mate-panel'
shared_module(
  'caminito-sound-applet',
  'sound-applet.c',
  link_with: [state, pulse, popup],
  dependencies: [gio, libpulse, libpulse_glib, applet_dep, common],
  gnu_symbol_visibility: 'hidden',
  install: true,
  install_dir: applet_dir,
)

# Its pins (CMN-217). The package's hook compiles the schemas, so nothing is compiled here.
install_data(
  'org.mate.panel.applet.caminito-sound.gschema.xml',
  install_dir: get_option('datadir') / 'glib-2.0' / 'schemas',
)

# The schema as glib-compile-schemas reads it, strictly.
glib_compile_schemas = find_program('glib-compile-schemas', required: false)
if glib_compile_schemas.found()
  test('schema', glib_compile_schemas,
       args: ['--strict', '--dry-run', meson.current_source_dir()])
endif

# Its entry in Add to Panel, in the normal directory, not Caminito's own: the applet works
# under X11 MATE too.
configure_file(
  input: 'org.mate.panel.CaminitoSoundApplet.mate-panel-applet.in',
  output: 'org.mate.panel.CaminitoSoundApplet.mate-panel-applet',
  configuration: {
    'APPLET_LOCATION': get_option('prefix') / applet_dir / 'libcaminito-sound-applet.so',
  },
  install: true,
  install_dir: get_option('datadir') / 'mate-panel' / 'applets',
)
