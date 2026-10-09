//! caminito-locker against Caminito (131, ADR 0007): the real locker, as `locker-command`
//! (126), takes the lock Caminito asks for, puts a lock surface on every output, and the
//! password typed at its dialog unlocks. It needs the locker Meson builds, named by
//! `CAMINITO_LOCKER`, and GTK with gtk-session-lock, so these are ignored by default and CI
//! runs them after the helpers' build.
//!
//! The fixture has no Wayland socket of its own, so `locker-command` is a shell that notes
//! its process and execs the locker on a socket of the test's, whose connections the test
//! hands to Caminito as that process's. The locker reads none of the user's settings:
//! GSettings is a memory backend on the test schemas, mate-screensaver's among them. Its
//! password is checked by a stand-in mate-screensaver service on pam_wrapper's pam_matrix,
//! from the modules directory `PAM_WRAPPER_MODULES` names, as in the conversation's own test
//! (helpers/caminito-locker/test-auth-pam.c). What the dialog shows is the checklist's: here
//! the locker's debug lines say what it did.

use std::io::ErrorKind;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::fixture::{Fixture, KEY_A, KEY_ENTER, test_schemas};
use crate::settings::Value;
use crate::settings::translate::CAMINITO;

/// The password of the stand-in service, typed as three presses of `KEY_A`.
const PASSWORD: &str = "aaa";

/// The debug line of a prompt the dialog shows, ready to be typed to.
const PROMPT: &str = "Setting prompt to: Password:";

/// caminito-locker as `locker-command`, in a scratch directory with its socket, its log,
/// the file its process is noted in, and the stand-in PAM service.
struct Locker {
    dir: tempfile::TempDir,
    listener: UnixListener,
}

impl Locker {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let listener = UnixListener::bind(dir.path().join("wayland-test")).unwrap();
        listener.set_nonblocking(true).unwrap();
        let locker = Locker { dir, listener };
        locker.write_stand_in_service();
        locker
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// pam_matrix's password file, with the test's user, and the service on it.
    fn write_stand_in_service(&self) {
        let modules = std::env::var("PAM_WRAPPER_MODULES")
            .expect("PAM_WRAPPER_MODULES names pam_wrapper's modules directory");
        let passdb = self.path("passdb");
        let line = format!("{}:{PASSWORD}:mate-screensaver\n", users_name());
        std::fs::write(&passdb, line).unwrap();
        std::fs::create_dir(self.path("pam.d")).unwrap();
        let service = format!(
            "auth required {modules}/pam_matrix.so passdb={passdb}\n\
             account required {modules}/pam_matrix.so passdb={passdb}\n",
            passdb = passdb.display(),
        );
        std::fs::write(self.path("pam.d").join("mate-screensaver"), service).unwrap();
    }

    /// The `locker-command`: a shell that notes its process, then execs caminito-locker in
    /// it, on the test's socket and settings, with its output in the log.
    fn command(&self) -> Vec<String> {
        let program = std::env::var("CAMINITO_LOCKER")
            .expect("CAMINITO_LOCKER names the built caminito-locker");
        let quoted = |path: &Path| format!("'{}'", path.display());
        let env = [
            ("XDG_RUNTIME_DIR", quoted(self.dir.path())),
            ("WAYLAND_DISPLAY", "wayland-test".into()),
            ("GDK_BACKEND", "wayland".into()),
            ("GSETTINGS_BACKEND", "memory".into()),
            ("GSETTINGS_SCHEMA_DIR", quoted(test_schemas())),
            ("NO_AT_BRIDGE", "1".into()),
            ("GIO_USE_VFS", "local".into()),
            ("LD_PRELOAD", "libpam_wrapper.so".into()),
            ("PAM_WRAPPER", "1".into()),
            ("PAM_WRAPPER_SERVICE_DIR", quoted(&self.path("pam.d"))),
        ];
        let env: Vec<String> = env
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect();
        let script = format!(
            "echo $$ >{pid}; exec env -u DISPLAY -u WAYLAND_SOCKET {env} '{program}' --verbose >>{log} 2>&1",
            pid = quoted(&self.path("pid")),
            env = env.join(" "),
            log = quoted(&self.path("log")),
        );
        vec!["sh".into(), "-c".into(), script]
    }

    /// The process caminito-locker runs in, once it's started.
    fn process(&self) -> Option<u32> {
        let text = std::fs::read_to_string(self.path("pid")).ok()?;
        text.strip_suffix('\n')?.parse().ok()
    }

    /// Waits for caminito-locker's connection, and hands it to Caminito as its process's.
    fn connect(&self, f: &mut Fixture) {
        f.dispatch_until_mut(|f| {
            let Some(pid) = self.process() else {
                return false;
            };
            match self.listener.accept() {
                Ok((stream, _)) => {
                    f.insert_stream(stream, Some(pid));
                    true
                }
                Err(err) if err.kind() == ErrorKind::WouldBlock => false,
                Err(err) => panic!("can't accept caminito-locker's connection: {err}"),
            }
        });
    }

    /// What caminito-locker wrote so far.
    fn seen(&self) -> String {
        std::fs::read_to_string(self.path("log")).unwrap_or_default()
    }

    /// How many times caminito-locker has written `line`.
    fn count(&self, line: &str) -> usize {
        self.seen().matches(line).count()
    }
}

impl Drop for Locker {
    fn drop(&mut self) {
        if let Some(pid) = self
            .process()
            .and_then(|pid| rustix::process::Pid::from_raw(pid as i32))
        {
            let _ = rustix::process::kill_process(pid, rustix::process::Signal::KILL);
        }
    }
}

/// The name of the user the test runs as, whom the locker asks PAM about.
fn users_name() -> String {
    let output = Command::new("id").arg("-un").output().unwrap();
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

/// Caminito with `outputs` and caminito-locker as its `locker-command`, asked to lock, with
/// the locker connected.
fn locked_by_caminito_locker(outputs: &[(i32, i32)]) -> (Fixture, Locker) {
    let locker = Locker::new();
    let command = Value::StrList(locker.command());
    let mut f = Fixture::with_settings(vec![(CAMINITO, "locker-command".into(), command)]);
    for &size in outputs {
        f.add_output(size, 1.0);
    }
    f.dispatch();
    assert!(f.state.request_lock());
    locker.connect(&mut f);
    (f, locker)
}

fn dump(f: &Fixture) -> serde_json::Value {
    serde_json::from_str(&f.state.caminito.state_dump()).unwrap()
}

/// Whether each output in the dump has a lock surface.
fn lock_surfaces(f: &Fixture) -> Vec<bool> {
    let dump = dump(f);
    let outputs = dump["outputs"].as_array().unwrap();
    let surface = |output: &serde_json::Value| output["lock_surface"].as_bool().unwrap();
    outputs.iter().map(surface).collect()
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

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; CI runs it"]
fn caminito_locker_locks_with_a_lock_surface_on_every_output() {
    let (mut f, locker) = locked_by_caminito_locker(&[(1920, 1080), (2560, 1440)]);

    f.dispatch_until_explained(
        |f| f.lock_state() == "locked" && lock_surfaces(f) == [true, true],
        || locker.seen(),
    );
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; CI runs it"]
fn the_password_typed_at_caminito_lockers_dialog_unlocks_after_a_wrong_one() {
    let (mut f, locker) = locked_by_caminito_locker(&[(1920, 1080)]);
    let seen = || locker.seen();
    f.dispatch_until_explained(|f| f.lock_state() == "locked", seen);
    f.dispatch_until_explained(|_| locker.count(PROMPT) == 1, seen);

    type_and_enter(&mut f, "a");
    f.dispatch_until_explained(|_| locker.count(PROMPT) == 2, seen);
    let refused = "Verify user returned error: Incorrect password.";
    assert!(locker.seen().contains(refused), "{}", locker.seen());
    assert_eq!(f.lock_state(), "locked");

    type_and_enter(&mut f, PASSWORD);
    // The locker unlocks and exits, and Caminito doesn't start it again (126).
    f.dispatch_until_explained(
        |f| f.lock_state() == "unlocked" && dump(f)["locker"]["state"] == "idle",
        seen,
    );
}
