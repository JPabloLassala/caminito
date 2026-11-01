# The stand-in service's directory, pam_wrapper's PAM_WRAPPER_SERVICE_DIR: the service
# alone, as pam_wrapper reads the whole directory.
configure_file(
  input: 'caminito-locker.in',
  output: 'caminito-locker',
  configuration: {'MODULES': pam_modules, 'PASSDB': passdb, 'FAILLOCK': faillock_dir},
)
