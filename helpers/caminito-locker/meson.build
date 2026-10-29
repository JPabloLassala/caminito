# The lock screen's locker (12, ADR 0007): mate-screensaver's unlock dialog on
# gtk-session-lock. Its own project (15), which the top-level meson.build includes as a
# subproject and installs.
project(
  'caminito-locker',
  'c',
  version: '0.1.0',
  license: 'GPL-3.0-or-later',
  meson_version: '>= 1.0',
  default_options: ['warning_level=2', 'c_std=gnu11'],
)

gnome = import('gnome')

glib = dependency('glib-2.0')
gio = dependency('gio-2.0')
pam = dependency('pam')

# What mate-screensaver's configure defines for the ported files. The dialog's themes are
# mate-screensaver's installed ones, and its strings are mate-screensaver's, translated by
# its catalogue.
config = configuration_data()
config.set_quoted('VERSION', meson.project_version())
config.set_quoted('GETTEXT_PACKAGE', 'mate-screensaver')
config.set_quoted('MATELOCALEDIR', get_option('prefix') / get_option('localedir'))
config.set_quoted('PAM_SERVICE_NAME', 'mate-screensaver')
config.set_quoted(
  'GTKBUILDERDIR',
  get_option('prefix') / get_option('datadir') / 'mate-screensaver',
)
configure_file(output: 'config.h', configuration: config)

auth = static_library(
  'gs-auth-pam',
  'gs-auth-pam.c',
  dependencies: [glib, pam],
)

# The session's status message, from mate-session's presence (132).
watcher = static_library('gs-watcher', 'gs-watcher.c', dependencies: gio)

# gtk-session-lock is barely maintained (ADR 0007); it builds at 16's baseline as it is, so
# it isn't vendored. libmate-desktop has MateBG, for the background (132), and the user
# switch's spawn. There's no libmatekbd: the layout indicator is X11's alone.
executable(
  'caminito-locker',
  'caminito-locker.c',
  'gs-lock-plug.c',
  'gs-prefs.c',
  gnome.compile_resources('caminito-locker-resources', 'caminito-locker.gresource.xml'),
  link_with: [auth, watcher],
  dependencies: [
    gio,
    dependency('gtk+-3.0'),
    dependency('gtk-session-lock-0'),
    dependency('mate-desktop-2.0'),
  ],
  install: true,
)

# The status message against a stand-in presence, on a bus of the test's own.
test(
  'watcher',
  executable('test-watcher', 'test-watcher.c', link_with: watcher, dependencies: gio),
)

# The conversation against a stand-in service, where pam_wrapper is installed to put one
# in mate-screensaver's place.
pam_wrapper = dependency('pam_wrapper', required: false)
if pam_wrapper.found()
  pam_modules = pam_wrapper.get_variable(pkgconfig: 'modules')
  passdb = meson.current_build_dir() / 'test-passdb'
  subdir('test-pam')
  test(
    'auth-pam',
    executable(
      'test-auth-pam',
      'test-auth-pam.c',
      link_with: auth,
      dependencies: glib,
    ),
    args: [passdb],
    env: {
      'LD_PRELOAD': 'libpam_wrapper.so',
      'PAM_WRAPPER': '1',
      'PAM_WRAPPER_SERVICE_DIR': meson.current_build_dir() / 'test-pam',
    },
  )
endif
