# The lock screen's locker (12, ADR 0007, ADR 0056): an unlock dialog derived from MATE's, on
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
i18n = import('i18n')

glib = dependency('glib-2.0')
gio = dependency('gio-2.0')
pam = dependency('pam')

# What the ported files need defined. The dialog's themes are in Caminito's data directory, and
# its strings are in the locker's own translation domain (ADR 0056).
config = configuration_data()
config.set_quoted('VERSION', meson.project_version())
config.set_quoted('GETTEXT_PACKAGE', 'caminito-locker')
config.set_quoted('MATELOCALEDIR', get_option('prefix') / get_option('localedir'))
config.set_quoted('PAM_SERVICE_NAME', 'caminito-locker')
config.set_quoted(
  'GTKBUILDERDIR',
  get_option('prefix') / get_option('datadir') / 'caminito',
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
# it isn't vendored. libmate-desktop has MateBG, for the background (132). There's no libmatekbd: the layout indicator reads org.mate.Caminito (CMN-299).
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

# The command MATE's components lock through, by name (CMN-297). The session command's PATH has
# this directory first, whatever the libexec directory of the distribution: the path is in
# caminito/src/session.rs, as Caminito's data directory is.
screensaver_command = executable(
  'mate-screensaver-command',
  'mate-screensaver-command.c',
  dependencies: gio,
  install: true,
  install_dir: get_option('prefix') / 'lib' / 'caminito' / 'bin',
)

# mate-panel offers Lock Screen only with this on PATH too. It has nothing to open.
install_data(
  'mate-screensaver-preferences',
  install_dir: get_option('prefix') / 'lib' / 'caminito' / 'bin',
  install_mode: 'rwxr-xr-x',
)

test(
  'screensaver-command',
  executable('test-screensaver-command', 'test-screensaver-command.c', dependencies: gio),
  args: [screensaver_command],
)

# The status message against a stand-in presence, on a bus of the test's own.
test(
  'watcher',
  executable('test-watcher', 'test-watcher.c', link_with: watcher, dependencies: gio),
)

# The conversation against a stand-in service, where pam_wrapper is installed to put one
# in the system's place.
pam_wrapper = dependency('pam_wrapper', required: false)
if pam_wrapper.found()
  pam_modules = pam_wrapper.get_variable(pkgconfig: 'modules')
  passdb = meson.current_build_dir() / 'test-passdb'
  faillock_dir = meson.current_build_dir() / 'test-faillock'
  subdir('test-pam')
  test(
    'auth-pam',
    executable(
      'test-auth-pam',
      'test-auth-pam.c',
      link_with: auth,
      dependencies: glib,
    ),
    args: [passdb, faillock_dir],
    env: {
      'LD_PRELOAD': 'libpam_wrapper.so',
      'PAM_WRAPPER': '1',
      'PAM_WRAPPER_SERVICE_DIR': meson.current_build_dir() / 'test-pam',
    },
  )
endif

# The strings the locker uses, copied from mate-screensaver's catalogues (po/README.md).
subdir('po')
