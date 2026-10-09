//! caminito-locker against Caminito (131, ADR 0007): the real locker takes the lock, puts a
//! lock surface on every output, and the password typed at its dialog unlocks. It needs
//! the locker Meson builds, named by `CAMINITO_LOCKER`, and GTK with gtk-session-lock, so
//! these are ignored by default and CI runs them after the helpers' build. Until
//! `locker-command` spawns it (126), a test starts it as a locker started by hand,
//! connected to a socket of the test's own. It reads none of the user's settings: GSettings
//! is a memory backend on the test schemas, mate-screensaver's among them.
//!
//! The password is checked by a stand-in mate-screensaver service on pam_wrapper's
//! pam_matrix, from the modules directory `PAM_WRAPPER_MODULES` names, as in the
//! conversation's own test (helpers/caminito-locker/test-auth-pam.c). What the dialog
//! shows is the checklist's: here the locker's debug lines say what it did.

use std::ffi::OsString;
use std::fs::File;
use std::io::ErrorKind;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::{Child, Command};

use super::fixture::{Fixture, KEY_A, KEY_ENTER, test_schemas};

/// The password of the stand-in service, typed as three presses of `KEY_A`.
const PASSWORD: &str = "aaa";

/// caminito-locker, running.
struct Locker {
    child: Child,
    /// What it wrote, for a failure to show.
    log: PathBuf,
    _dir: tempfile::TempDir,
}

impl Locker {
    /// Starts caminito-locker on `f`'s Caminito, and returns once it's connected.
    fn spawn(f: &mut Fixture) -> Self {
        let dir = tempfile::tempdir().unwrap();
        Self::spawn_with(f, dir, &[])
    }

    /// Starts caminito-locker as `spawn` does, with its PAM on the stand-in service.
    fn spawn_with_stand_in_service(f: &mut Fixture) -> Self {
        let modules = std::env::var("PAM_WRAPPER_MODULES")
            .expect("PAM_WRAPPER_MODULES names pam_wrapper's modules directory");
        let dir = tempfile::tempdir().unwrap();
        let passdb = dir.path().join("passdb");
        let name = users_name();
        std::fs::write(&passdb, format!("{name}:{PASSWORD}:mate-screensaver\n")).unwrap();
        let services = dir.path().join("pam.d");
        std::fs::create_dir(&services).unwrap();
        let service = format!(
            "auth required {modules}/pam_matrix.so passdb={passdb}\n\
             account required {modules}/pam_matrix.so passdb={passdb}\n",
            passdb = passdb.display(),
        );
        std::fs::write(services.join("mate-screensaver"), service).unwrap();
        let env = [
            ("LD_PRELOAD", "libpam_wrapper.so".into()),
            ("PAM_WRAPPER", "1".into()),
            ("PAM_WRAPPER_SERVICE_DIR", services.into_os_string()),
        ];
        Self::spawn_with(f, dir, &env)
    }

    fn spawn_with(f: &mut Fixture, dir: tempfile::TempDir, env: &[(&str, OsString)]) -> Self {
        let program = std::env::var_os("CAMINITO_LOCKER")
            .expect("CAMINITO_LOCKER names the built caminito-locker");
        let listener = UnixListener::bind(dir.path().join("wayland-test")).unwrap();
        listener.set_nonblocking(true).unwrap();
        let log = dir.path().join("log");
        let child = Command::new(program)
            .envs(env.iter().map(|(name, value)| (name, value)))
            .arg("--verbose")
            .env("XDG_RUNTIME_DIR", dir.path())
            .env("WAYLAND_DISPLAY", "wayland-test")
            .env_remove("WAYLAND_SOCKET")
            .env_remove("DISPLAY")
            .env("GDK_BACKEND", "wayland")
            .env("GSETTINGS_BACKEND", "memory")
            .env("GSETTINGS_SCHEMA_DIR", test_schemas())
            .env("NO_AT_BRIDGE", "1")
            .env("GIO_USE_VFS", "local")
            // GLib writes debug messages to stdout.
            .stdout(File::create(&log).unwrap())
            .stderr(File::options().append(true).open(&log).unwrap())
            .spawn()
            .expect("caminito-locker starts");
        let mut locker = Locker {
            child,
            log,
            _dir: dir,
        };
        f.dispatch_until_mut(|f| match listener.accept() {
            Ok((stream, _)) => {
                f.insert_stream(stream);
                true
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => {
                let status = locker.child.try_wait().unwrap();
                assert!(
                    status.is_none(),
                    "caminito-locker exited: {}",
                    locker.seen()
                );
                false
            }
            Err(err) => panic!("can't accept caminito-locker's connection: {err}"),
        });
        locker
    }

    /// What caminito-locker wrote so far.
    fn seen(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    /// How many times caminito-locker has written `line`.
    fn count(&self, line: &str) -> usize {
        self.seen().matches(line).count()
    }

    /// Whether caminito-locker exited with success, or `None` while it runs.
    fn exit_success(&mut self) -> Option<bool> {
        let status = self.child.try_wait().unwrap();
        status.map(|status| status.success())
    }
}

/// The name of the user the test runs as, whom the locker asks PAM about.
fn users_name() -> String {
    let output = Command::new("id").arg("-un").output().unwrap();
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// Types `text`, of `a`s alone, and Enter.
fn type_and_enter(f: &mut Fixture, text: &str) {
    assert!(text.chars().all(|c| c == 'a'), "only KEY_A is typed");
    for _ in text.chars() {
        f.key(KEY_A, true);
        f.key(KEY_A, false);
    }
    f.key(KEY_ENTER, true);
    f.key(KEY_ENTER, false);
}

/// The debug line of a prompt the dialog shows, ready to be typed to.
const PROMPT: &str = "Setting prompt to: Password:";

impl Drop for Locker {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Whether each output in the dump has a lock surface.
fn lock_surfaces(f: &Fixture) -> Vec<bool> {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let outputs = dump["outputs"].as_array().unwrap();
    let surface = |output: &serde_json::Value| output["lock_surface"].as_bool().unwrap();
    outputs.iter().map(surface).collect()
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, GTK and gtk-session-lock; CI runs it after Meson's build"]
fn caminito_locker_locks_with_a_lock_surface_on_every_output() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    f.add_output((2560, 1440), 1.0);
    assert_eq!(f.lock_state(), "unlocked");

    let locker = Locker::spawn(&mut f);
    let seen = || locker.seen();
    f.dispatch_until_explained(
        |f| f.lock_state() == "locked" && lock_surfaces(f) == [true, true],
        seen,
    );
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; CI runs it"]
fn the_password_typed_at_caminito_lockers_dialog_unlocks_after_a_wrong_one() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    let mut locker = Locker::spawn_with_stand_in_service(&mut f);
    {
        let seen = || locker.seen();
        f.dispatch_until_explained(|f| f.lock_state() == "locked", seen);
        f.dispatch_until_explained(|_| locker.count(PROMPT) == 1, seen);
    }

    type_and_enter(&mut f, "a");
    {
        let seen = || locker.seen();
        f.dispatch_until_explained(|_| locker.count(PROMPT) == 2, seen);
    }
    assert!(
        locker
            .seen()
            .contains("Verify user returned error: Incorrect password.")
    );
    assert_eq!(f.lock_state(), "locked");

    type_and_enter(&mut f, PASSWORD);
    f.dispatch_until_mut(|f| f.lock_state() == "unlocked" && locker.exit_success().is_some());
    assert_eq!(locker.exit_success(), Some(true), "{}", locker.seen());
}
