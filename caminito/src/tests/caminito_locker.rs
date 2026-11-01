//! caminito-locker against Caminito (131, ADR 0007): the real locker, as `locker-command`
//! (126), takes the lock Caminito asks for, puts a lock surface on every output, and the
//! password typed at its dialog unlocks. It needs the locker Meson builds, named by
//! `CAMINITO_LOCKER`, and GTK with gtk-session-lock, so these are ignored by default and
//! `tools/check` runs them after the helpers' build.
//!
//! The fixture has no Wayland socket of its own, so `locker-command` is a shell that notes
//! its process and execs the locker on a socket of the test's, whose connections the test
//! hands to Caminito as that process's. The locker reads none of the user's settings:
//! GSettings is a keyfile backend in the test's directory, on the test schemas,
//! org.mate.screensaver's and MateBG's among them, so a test can set keys and change them. Its
//! password is checked by a stand-in caminito-locker service on pam_wrapper's pam_matrix,
//! from the modules directory `PAM_WRAPPER_MODULES` names, as in the conversation's own test
//! (helpers/caminito-locker/test-auth-pam.c). What the dialog shows is the checklist's: here
//! the locker's debug lines say what it did.
//!
//! Its runtime directory is the test's too, where a test writes the file of each output's
//! wallpaper that caminito-wallpaperd would (218). No drawer runs, as if it had crashed.

use std::io::ErrorKind;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::Command;

use smithay::output::Output;

use super::fixture::{
    BTN_LEFT, Fixture, KEY_A, KEY_ENTER, KEY_ESC, KEY_LEFTALT, KEY_LEFTSHIFT, test_schemas,
};
use super::keymap::strings;
use super::locker::fixture_with;
use crate::output::output_geometry;
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, KEYBOARD_XKB};

/// The password of the stand-in service, typed as three presses of `KEY_A`.
const PASSWORD: &str = "aaa";

/// The debug line of a prompt the dialog shows, ready to be typed to.
const PROMPT: &str = "Setting prompt to: Password:";

/// caminito-locker as `locker-command`, in a scratch directory with its socket, its log,
/// the file its process is noted in, and the stand-in PAM service.
struct Locker {
    dir: tempfile::TempDir,
    listener: UnixListener,
    /// How long the dialog waits for a key before it hides, if not a minute.
    dialog_timeout_msec: Option<u32>,
    /// More of the locker's environment.
    env: Vec<(&'static str, String)>,
}

impl Locker {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let listener = UnixListener::bind(dir.path().join("wayland-test")).unwrap();
        listener.set_nonblocking(true).unwrap();
        let locker = Locker {
            dir,
            listener,
            dialog_timeout_msec: None,
            env: Vec::new(),
        };
        locker.write_stand_in_service();
        locker
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// The stand-in PAM service's file.
    fn service(&self) -> PathBuf {
        self.path("pam.d").join("caminito-locker")
    }

    /// pam_matrix's password file, with the test's user, and the service on it, with
    /// pam_faillock's tally in `faillock` as `system-auth`'s has it.
    fn write_stand_in_service(&self) {
        let modules = std::env::var("PAM_WRAPPER_MODULES")
            .expect("PAM_WRAPPER_MODULES names pam_wrapper's modules directory");
        // Without it the locker checks the password against the real service, and the
        // test only times out at the right password.
        assert!(
            Path::new(&modules).join("pam_matrix.so").exists(),
            "no pam_matrix.so in PAM_WRAPPER_MODULES ({modules:?}): is pam_wrapper installed?",
        );
        let passdb = self.path("passdb");
        let line = format!("{}:{PASSWORD}:caminito-locker\n", users_name());
        std::fs::write(&passdb, line).unwrap();
        std::fs::create_dir(self.path("pam.d")).unwrap();
        std::fs::create_dir(self.path("faillock")).unwrap();
        let service = format!(
            "auth required pam_faillock.so preauth silent nodelay dir={faillock}\n\
             auth [success=1 default=bad] {modules}/pam_matrix.so passdb={passdb}\n\
             auth [default=die] pam_faillock.so authfail nodelay dir={faillock}\n\
             auth sufficient pam_faillock.so authsucc dir={faillock}\n\
             account required {modules}/pam_matrix.so passdb={passdb}\n",
            passdb = passdb.display(),
            faillock = self.path("faillock").display(),
        );
        std::fs::write(self.service(), service).unwrap();
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
            ("GSETTINGS_BACKEND", "keyfile".into()),
            ("XDG_CONFIG_HOME", quoted(&self.path("config"))),
            ("GSETTINGS_SCHEMA_DIR", quoted(test_schemas())),
            ("NO_AT_BRIDGE", "1".into()),
            ("GIO_USE_VFS", "local".into()),
            ("LD_PRELOAD", "libpam_wrapper.so".into()),
            ("PAM_WRAPPER", "1".into()),
            // glibc fills freed memory, so a prompt read after it's freed never looks right.
            ("MALLOC_PERTURB_", "165".into()),
            ("PAM_WRAPPER_SERVICE_DIR", quoted(&self.path("pam.d"))),
        ];
        let mut env: Vec<(&str, String)> = env.to_vec();
        if let Some(msec) = self.dialog_timeout_msec {
            env.push(("CAMINITO_LOCKER_DIALOG_TIMEOUT_MSEC", msec.to_string()));
        }
        env.extend(self.env.iter().cloned());
        let env: Vec<String> = env
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect();
        let script = format!(
            "echo $$ >{pid}; exec env -u DISPLAY -u WAYLAND_SOCKET -u XDG_SEAT_PATH {env} '{program}' --verbose >>{log} 2>&1",
            pid = quoted(&self.path("pid")),
            env = env.join(" "),
            log = quoted(&self.path("log")),
        );
        vec!["sh".into(), "-c".into(), script]
    }

    /// Sets the locker's GSettings to `keyfile`, a keyfile backend's file, at the start or
    /// while it runs.
    fn set_settings(&self, keyfile: &str) {
        let dir = self.path("config").join("glib-2.0").join("settings");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("keyfile"), keyfile).unwrap();
    }

    /// Writes caminito-wallpaperd's file of each output's wallpaper (218), whole.
    fn set_wallpapers(&self, text: &str) {
        let dir = self.path("caminito-wallpaper");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("lock-screen.partial"), text).unwrap();
        std::fs::rename(dir.join("lock-screen.partial"), dir.join("lock-screen")).unwrap();
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
    locked_by_caminito_locker_with(outputs, "")
}

/// As `locked_by_caminito_locker`, with the locker's GSettings set to `keyfile`.
fn locked_by_caminito_locker_with(outputs: &[(i32, i32)], keyfile: &str) -> (Fixture, Locker) {
    locked_with_wallpapers(outputs, keyfile, |_| None)
}

/// As `locked_by_caminito_locker_with`, with the wallpaper file `wallpapers` gives for the
/// outputs, if any, written before the lock.
fn locked_with_wallpapers(
    outputs: &[(i32, i32)],
    keyfile: &str,
    wallpapers: impl FnOnce(&[Output]) -> Option<String>,
) -> (Fixture, Locker) {
    locked_with(outputs, keyfile, None, wallpapers)
}

/// As `locked_with_wallpapers`, with the dialog's timeout `dialog_timeout_msec` if given.
fn locked_with(
    outputs: &[(i32, i32)],
    keyfile: &str,
    dialog_timeout_msec: Option<u32>,
    wallpapers: impl FnOnce(&[Output]) -> Option<String>,
) -> (Fixture, Locker) {
    let mut locker = Locker::new();
    locker.dialog_timeout_msec = dialog_timeout_msec;
    locker.set_settings(keyfile);
    lock_with(locker, outputs, wallpapers)
}

/// Caminito with `outputs` and `locker` as its `locker-command`, asked to lock.
fn lock_with(
    locker: Locker,
    outputs: &[(i32, i32)],
    wallpapers: impl FnOnce(&[Output]) -> Option<String>,
) -> (Fixture, Locker) {
    let command = Value::StrList(locker.command());
    let mut f = Fixture::with_settings(vec![(CAMINITO, "locker-command".into(), command)]);
    let added: Vec<Output> = outputs
        .iter()
        .map(|&size| f.add_output(size, 1.0))
        .collect();
    f.dispatch();
    if let Some(text) = wallpapers(&added) {
        locker.set_wallpapers(&text);
    }
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

/// Presses and lets go of `key`.
fn tap(f: &mut Fixture, key: u32) {
    f.key(key, true);
    f.key(key, false);
}

/// Types `text`, of `a`s alone, and Enter.
fn type_and_enter(f: &mut Fixture, text: &str) {
    assert!(text.chars().all(|c| c == 'a'), "only KEY_A is typed");
    for _ in text.chars() {
        tap(f, KEY_A);
    }
    tap(f, KEY_ENTER);
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn caminito_locker_locks_with_a_lock_surface_on_every_output() {
    let (mut f, locker) = locked_by_caminito_locker(&[(1920, 1080), (2560, 1440)]);

    f.dispatch_until_explained(
        |f| f.lock_state() == "locked" && lock_surfaces(f) == [true, true],
        || locker.seen(),
    );
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
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

/// `keyfile`'s locker on one output, with `env`, asked to lock with the locker connected.
fn locked_with_env(keyfile: &str, env: Vec<(&'static str, String)>) -> (Fixture, Locker) {
    let mut locker = Locker::new();
    locker.env = env;
    locker.set_settings(keyfile);
    lock_with(locker, &[(1920, 1080)], |_| None)
}

/// The Spanish catalogue of caminito-locker's domain, built from its `.po` file under `dir`,
/// as the package's would be under the locale directory.
fn write_spanish_catalogue(dir: &Path) {
    let messages = dir.join("es").join("LC_MESSAGES");
    std::fs::create_dir_all(&messages).unwrap();
    let po = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../helpers/caminito-locker/po/es.po"
    );
    let status = Command::new("msgfmt")
        .arg("-o")
        .arg(messages.join("caminito-locker.mo"))
        .arg(po)
        .status()
        .expect("msgfmt runs");
    assert!(status.success());
}

/// A UTF-8 locale of this machine's other than C's, under which glibc honours `LANGUAGE`.
fn a_utf8_locale() -> String {
    let output = Command::new("locale").arg("-a").output().unwrap();
    let locales = String::from_utf8(output.stdout).unwrap();
    locales
        .lines()
        .find(|locale| locale.ends_with("utf8") && !locale.starts_with("C."))
        .expect("the test needs a UTF-8 locale other than C.UTF-8 generated")
        .to_owned()
}

/// ADR 0056: nothing of the package is read. The schema is the one Caminito ships, a link in
/// the test schemas, the service is caminito-locker's, the theme directory is Caminito's, the
/// strings are the locker's own domain's, and the program names none of the package's files.
#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn caminito_locker_locks_and_unlocks_in_spanish_with_nothing_of_mate_screensaver() {
    let program = std::fs::read(std::env::var("CAMINITO_LOCKER").unwrap()).unwrap();
    // As a whole string, a service or a domain, or as a directory of a path. The name is also
    // in the dialog's notice, and can be in the build's directory.
    for name in [&b"\0mate-screensaver\0"[..], &b"/mate-screensaver"[..]] {
        assert!(!program.windows(name.len()).any(|bytes| bytes == name));
    }

    let catalogues = tempfile::tempdir().unwrap();
    write_spanish_catalogue(catalogues.path());
    let locale_dir = catalogues.path().display().to_string();
    let env = vec![
        ("LC_ALL", a_utf8_locale()),
        ("LANGUAGE", "es".to_owned()),
        ("CAMINITO_LOCKER_LOCALE_DIR", format!("'{locale_dir}'")),
    ];
    let (mut f, locker) = locked_with_env("", env);
    let seen = || locker.seen();
    let prompt = "Setting prompt to: Contraseña:";
    f.dispatch_until_explained(|f| f.lock_state() == "locked", seen);
    f.dispatch_until_explained(|_| locker.count(prompt) == 1, seen);
    assert!(
        seen().contains("/caminito/lock-dialog-default.ui"),
        "{}",
        seen()
    );

    type_and_enter(&mut f, "a");
    f.dispatch_until_explained(|_| locker.count(prompt) == 2, seen);
    // Not "Incorrect password.": the locker tells that by the prompt, in the language of
    // its catalogue, and the stand-in's pam_matrix asks in English.
    let refused = "Verify user returned error: La autenticación ha fallado.";
    assert!(seen().contains(refused), "{}", seen());

    type_and_enter(&mut f, PASSWORD);
    f.dispatch_until_explained(
        |f| f.lock_state() == "unlocked" && dump(f)["locker"]["state"] == "idle",
        seen,
    );
}

const SWITCH_ENABLED: &str = "[org/mate/screensaver]\nuser-switch-enabled=true\n";

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn switch_user_hides_without_a_display_managers_seat() {
    let (mut f, locker) = locked_with_env(SWITCH_ENABLED, Vec::new());
    let seen = || locker.seen();
    f.dispatch_until_explained(|f| f.lock_state() == "locked", seen);
    f.dispatch_until_explained(|_| locker.count("Switch User: enabled") >= 1, seen);
    assert_eq!(locker.count("Switch User button: shown"), 0, "{}", seen());
    assert!(
        locker.count("Switch User button: hidden") >= 1,
        "{}",
        seen()
    );
}

/// A seat path that no display manager answers for, as after a greeter's gone: the system bus
/// here is one that can't be reached. That a seat which answers shows the button is the
/// checklist's, with the real greeter.
#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn switch_user_hides_with_a_seat_that_no_display_manager_answers_for() {
    let env = vec![
        (
            "XDG_SEAT_PATH",
            "/org/freedesktop/DisplayManager/Seat0".to_owned(),
        ),
        (
            "DBUS_SYSTEM_BUS_ADDRESS",
            "unix:path=/nonexistent".to_owned(),
        ),
    ];
    let (mut f, locker) = locked_with_env(SWITCH_ENABLED, env);
    let seen = || locker.seen();
    f.dispatch_until_explained(|f| f.lock_state() == "locked", seen);
    f.dispatch_until_explained(|_| locker.count("Switch User: enabled") >= 1, seen);
    assert_eq!(locker.count("Switch User button: shown"), 0, "{}", seen());
    assert!(
        locker.count("Switch User button: hidden") >= 1,
        "{}",
        seen()
    );
}

/// ADR 0056: a missing service is a failed start, not a prompt that can't succeed. The locker
/// exits each time, so Caminito gives up, the session stays locked, and the message says how
/// to end it.
#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn a_missing_pam_service_reaches_the_give_up_state_with_the_session_locked() {
    let locker = Locker::new();
    std::fs::remove_file(locker.service()).unwrap();
    let mut f = fixture_with(locker.command(), Vec::new());

    f.state.request_lock();
    f.dispatch_until_explained(
        |f| dump(f)["locker"]["state"] == "gave-up",
        || locker.seen(),
    );

    assert_eq!(f.lock_state(), "pending");
    assert_eq!(
        locker.count("has no PAM service caminito-locker"),
        5,
        "{}",
        locker.seen()
    );
    let message = dump(&f)["locker"]["message"].as_str().unwrap().to_owned();
    assert!(message.contains("loginctl unlock-session"), "{message}");
}

/// The failures `pam_faillock` has recorded for the user in the stand-in service's tally.
fn faillock_records(locker: &Locker) -> usize {
    std::fs::read_to_string(locker.path("faillock").join(users_name()))
        .map(|tally| {
            tally
                .lines()
                .filter(|line| !line.is_empty() && !line.starts_with('#'))
                .count()
        })
        .unwrap_or(0)
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn a_dialog_that_times_out_leaves_the_conversation_open_and_counts_no_failure() {
    let (mut f, locker) = locked_with(&[(1920, 1080)], "", Some(500), |_| None);
    let seen = || locker.seen();
    f.dispatch_until_explained(|f| f.lock_state() == "locked", seen);
    f.dispatch_until_explained(|_| locker.count(PROMPT) == 1, seen);

    // Typed and left: the timeout hides the dialog with its entry cleared. The conversation
    // is the same one, still waiting: PAM has no result, so pam_faillock has no record.
    tap(&mut f, KEY_A);
    f.dispatch_until_explained(|_| locker.count(DIALOG_SHOWS) == 1, seen);
    f.dispatch_until_explained(|_| locker.count(DIALOG_HIDES) == 1, seen);
    // Three of the timeouts: a cancel that answered PAM would restart the conversation at
    // once, and its failure would be recorded well before this ends.
    f.dispatch_for(std::time::Duration::from_millis(1500));
    assert_eq!(
        locker.count(DIALOG_HIDES),
        1,
        "the hidden dialog has no timeout: {}",
        seen()
    );
    assert_eq!(locker.count("Starting conversation"), 1, "{}", seen());
    assert_eq!(locker.count("Verify user returned"), 0, "{}", seen());
    assert_eq!(faillock_records(&locker), 0, "{}", seen());
    assert_eq!(f.lock_state(), "locked");

    // A password typed from the Idle view unlocks: the `a` from before isn't in the entry.
    type_and_enter(&mut f, PASSWORD);
    f.dispatch_until_explained(
        |f| f.lock_state() == "unlocked" && dump(f)["locker"]["state"] == "idle",
        seen,
    );
    assert_eq!(locker.count(PROMPT), 1, "{}", seen());
    assert_eq!(locker.count("Starting conversation"), 1, "{}", seen());
    assert_eq!(faillock_records(&locker), 0, "{}", seen());
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn a_click_shows_the_dialog_and_motion_does_not() {
    let (mut f, locker, output) = idle_with_prompt(None);

    f.pointer_move(100.0, 100.0);
    f.pointer_move(300.0, 200.0);
    // Nothing should happen: a key's dialog shows within a few frames, far sooner than this.
    f.dispatch_for(std::time::Duration::from_millis(300));
    assert_eq!(locker.count(DIALOG_SHOWS), 0, "{}", locker.seen());
    wait_for_no_dialog_on(&mut f, &output);

    f.pointer_button(BTN_LEFT, true);
    f.pointer_button(BTN_LEFT, false);
    wait_for_dialog_on(&mut f, &locker, &output);
    assert_eq!(
        locker.count("Starting conversation"),
        1,
        "{}",
        locker.seen()
    );
    assert_eq!(faillock_records(&locker), 0, "{}", locker.seen());

    // With focus in the entry.
    type_and_enter(&mut f, PASSWORD);
    f.dispatch_until_explained(|f| f.lock_state() == "unlocked", || locker.seen());
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn a_modifier_or_esc_shows_the_dialog_with_nothing_typed() {
    for key in [KEY_LEFTSHIFT, KEY_ESC] {
        let (mut f, locker, output) = idle_with_prompt(None);

        tap(&mut f, key);
        wait_for_dialog_on(&mut f, &locker, &output);

        // A character typed by the key would make this password wrong.
        type_and_enter(&mut f, PASSWORD);
        f.dispatch_until_explained(|f| f.lock_state() == "unlocked", || locker.seen());
    }
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn hiding_and_showing_the_dialog_many_times_counts_no_failure() {
    let (mut f, locker, _output) = idle_with_prompt(None);

    for times in 1..=25 {
        tap(&mut f, KEY_A);
        f.dispatch_until_explained(|_| locker.count(DIALOG_SHOWS) == times, || locker.seen());
        tap(&mut f, KEY_ESC);
        f.dispatch_until_explained(|_| locker.count(DIALOG_HIDES) == times, || locker.seen());
    }

    assert_eq!(
        locker.count("Starting conversation"),
        1,
        "{}",
        locker.seen()
    );
    assert_eq!(locker.count("Verify user returned"), 0, "{}", locker.seen());
    assert_eq!(faillock_records(&locker), 0, "{}", locker.seen());

    type_and_enter(&mut f, PASSWORD);
    f.dispatch_until_explained(|f| f.lock_state() == "unlocked", || locker.seen());
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn the_dialog_follows_the_pointer_and_survives_its_outputs_removal() {
    let (mut f, locker) =
        locked_by_caminito_locker_with(&[(1920, 1080), (1920, 1080)], SOLID_BACKGROUND);
    f.dispatch_until_explained(|f| f.lock_state() == "locked", || locker.seen());
    f.dispatch_until_explained(|_| locker.count(PROMPT) == 1, || locker.seen());
    let outputs = f.state.caminito.outputs.clone();

    // Hidden, it follows too, and shows on the output the pointer chose.
    f.pointer_move(2020.0, 100.0);
    // Nothing should show: a key's dialog shows within a few frames, far sooner than this.
    f.dispatch_for(std::time::Duration::from_millis(200));
    assert_eq!(locker.count(DIALOG_SHOWS), 0, "{}", locker.seen());
    tap(&mut f, KEY_LEFTSHIFT);
    wait_for_dialog_on(&mut f, &locker, &outputs[1]);
    wait_for_no_dialog_on(&mut f, &outputs[0]);

    f.pointer_move(100.0, 100.0);
    wait_for_dialog_on(&mut f, &locker, &outputs[0]);
    wait_for_no_dialog_on(&mut f, &outputs[1]);

    // Its output goes, with the dialog showing: it goes on on the other, and takes the
    // password there.
    f.unplug(&outputs[0]);
    wait_for_dialog_on(&mut f, &locker, &outputs[1]);
    assert_eq!(f.lock_state(), "locked");
    type_and_enter(&mut f, PASSWORD);
    f.dispatch_until_explained(|f| f.lock_state() == "unlocked", || locker.seen());
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn a_module_that_gives_up_starts_the_next_conversation_quietly() {
    let (mut f, locker) = lock_with(
        locker_with_a_service_that_gives_up(),
        &[(1920, 1080)],
        |_| None,
    );
    f.dispatch_until_explained(|f| f.lock_state() == "locked", || locker.seen());
    let output = f.state.caminito.outputs[0].clone();

    // Sooner than the 3 s a message on show gets, and with nothing on show.
    let started = std::time::Instant::now();
    f.dispatch_until_explained(
        |_| locker.count("Starting conversation") >= 3,
        || locker.seen(),
    );
    assert!(
        started.elapsed() < std::time::Duration::from_secs(5),
        "{}",
        locker.seen()
    );
    assert_eq!(locker.count(DIALOG_SHOWS), 0, "{}", locker.seen());
    wait_for_no_dialog_on(&mut f, &output);

    // A key still shows the dialog, and the lock holds. Open, the next give-ups aren't quiet:
    // each leaves its message on show for its 3 s. One may be on its way as the key lands.
    let quiet = locker.count(QUIET);
    tap(&mut f, KEY_LEFTSHIFT);
    wait_for_dialog_on(&mut f, &locker, &output);
    assert_eq!(f.lock_state(), "locked");
    let starts = locker.count("Starting conversation");
    f.dispatch_until_explained(
        |_| locker.count("Starting conversation") == starts + 2,
        || locker.seen(),
    );
    assert!(locker.count(QUIET) <= quiet + 1, "{}", locker.seen());
}

/// As `Locker::new`, with a service that ends every conversation by itself, with no prompt.
fn locker_with_a_service_that_gives_up() -> Locker {
    let locker = Locker::new();
    locker.set_settings(SOLID_BACKGROUND);
    std::fs::write(
        locker.service(),
        "auth required pam_deny.so\naccount required pam_deny.so\n",
    )
    .unwrap();
    locker
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn esc_hides_the_dialog_with_no_prompt_up() {
    let (mut f, locker) = lock_with(
        locker_with_a_service_that_gives_up(),
        &[(1920, 1080)],
        |_| None,
    );
    f.dispatch_until_explained(|f| f.lock_state() == "locked", || locker.seen());

    tap(&mut f, KEY_LEFTSHIFT);
    f.dispatch_until_explained(|_| locker.count(DIALOG_SHOWS) == 1, || locker.seen());
    tap(&mut f, KEY_ESC);
    f.dispatch_until_explained(|_| locker.count(DIALOG_HIDES) == 1, || locker.seen());
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn conversations_that_keep_ending_do_not_put_off_the_dialogs_timeout() {
    // Longer than the 3 s between the conversations, each of which would start it again.
    let mut locker = locker_with_a_service_that_gives_up();
    locker.dialog_timeout_msec = Some(3500);
    let (mut f, locker) = lock_with(locker, &[(1920, 1080)], |_| None);
    f.dispatch_until_explained(|f| f.lock_state() == "locked", || locker.seen());

    tap(&mut f, KEY_LEFTSHIFT);
    f.dispatch_until_explained(|_| locker.count(DIALOG_SHOWS) == 1, || locker.seen());
    f.dispatch_until_explained(|_| locker.count(DIALOG_HIDES) == 1, || locker.seen());
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn esc_after_a_wrong_password_hides_the_dialog_and_the_next_prompt_stays_hidden() {
    let (mut f, locker, output) = idle_with_prompt(None);

    type_and_enter(&mut f, "a");
    f.dispatch_until_explained(
        |_| locker.seen().contains("Verify user returned error"),
        || locker.seen(),
    );
    tap(&mut f, KEY_ESC);
    f.dispatch_until_explained(|_| locker.count(DIALOG_HIDES) == 1, || locker.seen());
    f.dispatch_until_explained(|_| locker.count(PROMPT) == 2, || locker.seen());
    wait_for_no_dialog_on(&mut f, &output);

    type_and_enter(&mut f, PASSWORD);
    f.dispatch_until_explained(|f| f.lock_state() == "unlocked", || locker.seen());
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn every_output_has_a_clock_in_the_idle_view() {
    let (mut f, locker) =
        locked_by_caminito_locker_with(&[(1920, 1080), (1280, 720)], SOLID_BACKGROUND);
    f.dispatch_until_explained(|f| f.lock_state() == "locked", || locker.seen());

    // The clock's at the top in the middle, with the wallpaper around it.
    for output in f.state.caminito.outputs.clone() {
        let width = output_geometry(&output).size.w;
        wait_for_not_wallpaper(&mut f, &locker, &output, width / 2, 60);
        f.wait_for_pixel_on(&output, 20, 60, WALLPAPER);
    }
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn the_clock_is_smaller_only_on_an_output_too_short_for_it_and_the_dialog() {
    const SMALLER: &str = "The clock is smaller";
    const BACK: &str = "The clock is back at its size";
    let (mut f, locker) =
        locked_by_caminito_locker_with(&[(1920, 1080), (1024, 400)], SOLID_BACKGROUND);
    f.dispatch_until_explained(|f| f.lock_state() == "locked", || locker.seen());

    tap(&mut f, KEY_LEFTSHIFT);
    f.dispatch_until_explained(|_| locker.count(DIALOG_SHOWS) == 1, || locker.seen());
    // Nothing should change: the clock fits at the idle after the dialog's allocation, within
    // a few frames, far sooner than this.
    f.dispatch_for(std::time::Duration::from_millis(300));
    assert_eq!(locker.count(SMALLER), 0, "{}", locker.seen());

    f.pointer_move(2000.0, 100.0);
    f.dispatch_until_explained(|_| locker.count(SMALLER) == 1, || locker.seen());
    tap(&mut f, KEY_ESC);
    f.dispatch_until_explained(|_| locker.count(BACK) == 1, || locker.seen());
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn an_empty_answer_is_not_sent_to_pam() {
    let (mut f, locker) = locked_by_caminito_locker(&[(1920, 1080)]);
    let seen = || locker.seen();
    f.dispatch_until_explained(|f| f.lock_state() == "locked", seen);
    f.dispatch_until_explained(|_| locker.count(PROMPT) == 1, seen);

    // The first Enter shows the dialog, and the second answers it.
    for _ in 0..2 {
        tap(&mut f, KEY_ENTER);
    }
    f.dispatch_until_explained(
        |_| locker.count("An empty answer is not sent to PAM") == 1,
        seen,
    );
    // An answer sent to PAM comes back from pam_matrix in milliseconds.
    f.dispatch_for(std::time::Duration::from_millis(1000));
    assert_eq!(locker.count("got response"), 0, "{}", seen());
    assert_eq!(locker.count("Verify user returned"), 0, "{}", seen());
    assert_eq!(faillock_records(&locker), 0, "{}", seen());

    // The same prompt takes the password after it.
    type_and_enter(&mut f, PASSWORD);
    f.dispatch_until_explained(
        |f| f.lock_state() == "unlocked" && dump(f)["locker"]["state"] == "idle",
        seen,
    );
}

/// A background of one colour, `0x608040`, as MateBG has it with `solid` shading.
const SOLID_BACKGROUND: &str = "\
[org/mate/desktop/background]
picture-filename=''
color-shading-type='solid'
primary-color='#608040'

[org/mate/screensaver]
picture-filename=''
";

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn caminito_lockers_windows_show_the_desktop_background_on_every_output() {
    let (mut f, locker) =
        locked_by_caminito_locker_with(&[(1920, 1080), (2560, 1440)], SOLID_BACKGROUND);
    f.dispatch_until_explained(|f| f.lock_state() == "locked", || locker.seen());

    // Away from the dialog, in the middle of one output, and the pointer, at the origin.
    let outputs = f.state.caminito.outputs.clone();
    for output in &outputs {
        f.wait_for_pixel_on(output, 100, 100, 0x608040);
    }
}

/// The wallpaper's colour in `SOLID_BACKGROUND`.
const WALLPAPER: u32 = 0x608040;

/// The debug lines of the Unlock dialog showing and hiding.
const DIALOG_SHOWS: &str = "The Unlock dialog shows";
const DIALOG_HIDES: &str = "The Unlock dialog hides";

/// The debug line of a conversation that ended by itself with the dialog hidden.
const QUIET: &str = "The conversation ended with the dialog hidden";

/// Waits until `(x, y)` on `output` isn't the wallpaper, and fails rather than hangs.
#[track_caller]
fn wait_for_not_wallpaper(f: &mut Fixture, locker: &Locker, output: &Output, x: i32, y: i32) {
    let wallpaper = 0xff00_0000 | WALLPAPER;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    f.dispatch_until_mut(|f| {
        assert!(
            std::time::Instant::now() < deadline,
            "({x}, {y}) is still the wallpaper after 5 s: {}",
            locker.seen()
        );
        let at = (x, y).into();
        let pixel = f.state.backend.headless_mut().pixel(output, at);
        pixel.is_some_and(|pixel| pixel != wallpaper)
    });
}

/// Waits until the dialog covers the middle of `output`, where it shows.
#[track_caller]
fn wait_for_dialog_on(f: &mut Fixture, locker: &Locker, output: &Output) {
    let size = output_geometry(output).size;
    wait_for_not_wallpaper(f, locker, output, size.w / 2, size.h / 2);
}

/// Waits until the middle of `output` is the wallpaper's.
#[track_caller]
fn wait_for_no_dialog_on(f: &mut Fixture, output: &Output) {
    let size = output_geometry(output).size;
    f.wait_for_pixel_on(output, size.w / 2, size.h / 2, WALLPAPER);
}

/// The lock on one output with its prompt asked, in the Idle view.
fn idle_with_prompt(dialog_timeout_msec: Option<u32>) -> (Fixture, Locker, Output) {
    let (mut f, locker) = locked_with(
        &[(1920, 1080)],
        SOLID_BACKGROUND,
        dialog_timeout_msec,
        |_| None,
    );
    f.dispatch_until_explained(|f| f.lock_state() == "locked", || locker.seen());
    f.dispatch_until_explained(|_| locker.count(PROMPT) == 1, || locker.seen());
    let output = f.state.caminito.outputs[0].clone();
    wait_for_no_dialog_on(&mut f, &output);
    (f, locker, output)
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn a_lock_shows_no_dialog_and_a_key_shows_it_and_is_typed() {
    let (mut f, locker, output) = idle_with_prompt(None);
    assert_eq!(locker.count(DIALOG_SHOWS), 0, "{}", locker.seen());

    // The first key opens the dialog and goes in the entry: the rest of the password and
    // Enter unlock.
    tap(&mut f, KEY_A);
    wait_for_dialog_on(&mut f, &locker, &output);
    type_and_enter(&mut f, &PASSWORD[1..]);
    f.dispatch_until_explained(
        |f| f.lock_state() == "unlocked" && dump(f)["locker"]["state"] == "idle",
        || locker.seen(),
    );
    assert_eq!(locker.count(PROMPT), 1, "{}", locker.seen());
}

/// A PNG of one pixel, `0x406080`.
const PICTURE: [u8; 69] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x70, 0x48, 0x68, 0x00,
    0x00, 0x02, 0x04, 0x01, 0x21, 0xcb, 0xd7, 0x94, 0xa2, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e,
    0x44, 0xae, 0x42, 0x60, 0x82,
];

/// `SOLID_BACKGROUND`, with mate-screensaver's `picture-filename` set to `path`.
fn with_picture_filename(path: &Path) -> String {
    SOLID_BACKGROUND.replace(
        "[org/mate/screensaver]\npicture-filename=''",
        &format!(
            "[org/mate/screensaver]\npicture-filename='{}'",
            path.display()
        ),
    )
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn picture_filename_replaces_the_desktop_background_when_the_file_exists() {
    let dir = tempfile::tempdir().unwrap();
    let picture = dir.path().join("picture.png");
    std::fs::write(&picture, PICTURE).unwrap();
    let missing = dir.path().join("missing.png");

    // A picture-filename that isn't there leaves the desktop's.
    let keyfile = with_picture_filename(&missing);
    let (mut f, locker) = locked_by_caminito_locker_with(&[(1920, 1080)], &keyfile);
    f.dispatch_until_explained(|f| f.lock_state() == "locked", || locker.seen());
    f.wait_for_pixel(100, 100, 0x608040);
    drop((f, locker));

    let keyfile = with_picture_filename(&picture);
    let (mut f, locker) = locked_by_caminito_locker_with(&[(1920, 1080)], &keyfile);
    f.dispatch_until_explained(|f| f.lock_state() == "locked", || locker.seen());
    f.wait_for_pixel(100, 100, 0x406080);
}

/// The wallpaper file's group for `output` at `connector`, as caminito-wallpaperd writes
/// it: told by its make, model and place, showing `picture`, or no picture, over `colour`.
fn wallpaper(connector: &str, output: &Output, picture: Option<&Path>, colour: &str) -> String {
    let properties = output.physical_properties();
    let place = output_geometry(output).loc;
    let picture = picture.map(|path| path.display().to_string());
    format!(
        "[{connector}]\n\
         make={}\n\
         model={}\n\
         x={}\n\
         y={}\n\
         picture-filename={}\n\
         picture-options=zoom\n\
         primary-color={colour}\n\n",
        properties.make,
        properties.model,
        place.x,
        place.y,
        picture.unwrap_or_default(),
    )
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn each_output_shows_its_own_wallpaper_from_the_drawers_file() {
    let dir = tempfile::tempdir().unwrap();
    let picture = dir.path().join("picture.png");
    std::fs::write(&picture, PICTURE).unwrap();
    let (mut f, locker) =
        locked_with_wallpapers(&[(1920, 1080), (2560, 1440)], SOLID_BACKGROUND, |outputs| {
            Some(
                wallpaper("headless-1", &outputs[0], Some(&picture), "#000000")
                    + &wallpaper("headless-2", &outputs[1], None, "#804060"),
            )
        });
    f.dispatch_until_explained(|f| f.lock_state() == "locked", || locker.seen());

    let outputs = f.state.caminito.outputs.clone();
    f.wait_for_pixel_on(&outputs[0], 100, 100, 0x406080);
    f.wait_for_pixel_on(&outputs[1], 100, 100, 0x804060);
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn with_no_file_or_an_output_it_lacks_the_desktop_background_shows() {
    let (mut f, locker) = locked_by_caminito_locker_with(&[(1920, 1080)], SOLID_BACKGROUND);
    f.dispatch_until_explained(|f| f.lock_state() == "locked", || locker.seen());
    f.wait_for_pixel(100, 100, 0x608040);
    assert!(
        locker.seen().contains("No wallpaper file"),
        "{}",
        locker.seen()
    );
    drop((f, locker));

    // The file has the first output alone.
    let (mut f, locker) =
        locked_with_wallpapers(&[(1920, 1080), (2560, 1440)], SOLID_BACKGROUND, |outputs| {
            Some(wallpaper("headless-1", &outputs[0], None, "#804060"))
        });
    f.dispatch_until_explained(|f| f.lock_state() == "locked", || locker.seen());
    let outputs = f.state.caminito.outputs.clone();
    f.wait_for_pixel_on(&outputs[0], 100, 100, 0x804060);
    f.wait_for_pixel_on(&outputs[1], 100, 100, 0x608040);
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn a_picture_that_is_gone_leaves_the_desktop_background() {
    let dir = tempfile::tempdir().unwrap();
    let gone = dir.path().join("gone.png");
    let (mut f, locker) = locked_with_wallpapers(&[(1920, 1080)], SOLID_BACKGROUND, |outputs| {
        Some(wallpaper("headless-1", &outputs[0], Some(&gone), "#804060"))
    });
    f.dispatch_until_explained(|f| f.lock_state() == "locked", || locker.seen());

    f.wait_for_pixel(100, 100, 0x608040);
    assert!(
        locker.seen().contains("names a picture that is gone"),
        "{}",
        locker.seen()
    );
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn caminito_locker_follows_the_keys_of_its_buttons_and_status_message() {
    let keyfile = "\
[org/mate/screensaver]
user-switch-enabled=true
logout-enabled=true
logout-delay=0
logout-command='true'

[org/mate/desktop/lockdown]
disable-user-switching=true
";
    let (mut f, locker) = locked_by_caminito_locker_with(&[(1920, 1080)], keyfile);
    let seen = || locker.seen();
    f.dispatch_until_explained(|f| f.lock_state() == "locked", seen);
    f.dispatch_until_explained(|_| locker.count("Switch User: disabled") == 1, seen);
    assert!(
        locker.seen().contains("Log Out: shown"),
        "{}",
        locker.seen()
    );
    // No mate-session on the test's bus, so no message.
    assert!(
        locker.seen().contains("Status message: (none)"),
        "{}",
        locker.seen()
    );

    // The keys are followed while locked, as mate-screensaver's daemon follows them.
    locker.set_settings(
        "\
[org/mate/screensaver]
user-switch-enabled=true
logout-enabled=true
logout-delay=0
logout-command=''
",
    );
    f.dispatch_until_explained(|_| locker.count("Switch User: enabled") >= 1, seen);
    f.dispatch_until_explained(|_| locker.count("Log Out: hidden") >= 1, seen);
}

/// The debug line of the keyboard layout the dialog's indicator shows.
fn indicator(name: &str) -> String {
    format!("Keyboard layout indicator: {name}")
}

/// The debug line of a hidden keyboard layout indicator.
const INDICATOR_HIDDEN: &str = "Keyboard layout indicator: (hidden)";

/// Caminito with `names` as its layouts, switched by Alt+Shift, and caminito-locker as its
/// `locker-command`, asked to lock. `start_dbus` has Caminito answer on the bus or not, as
/// it needs the name that the Locker looks for.
fn locked_with_layouts(names: &[&str], start_dbus: fn(&mut Fixture)) -> (Fixture, Locker) {
    let locker = Locker::new();
    locker.set_settings("");
    let options = strings(&["grp\tgrp:alt_shift_toggle"]);
    let mut f = Fixture::with_settings(vec![
        (
            CAMINITO,
            "locker-command".into(),
            Value::StrList(locker.command()),
        ),
        (KEYBOARD_XKB, "layouts".into(), strings(names)),
        (KEYBOARD_XKB, "options".into(), options),
    ]);
    f.add_output((1920, 1080), 1.0);
    f.dispatch();
    start_dbus(&mut f);
    assert!(f.state.request_lock());
    locker.connect(&mut f);
    (f, locker)
}

fn answering(f: &mut Fixture) {
    f.start_dbus_with_names();
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn the_layout_indicator_shows_with_two_layouts_and_follows_the_switch() {
    let (mut f, locker) = locked_with_layouts(&["us", "es"], answering);
    let seen = || locker.seen();
    f.dispatch_until_explained(|_| locker.count(&indicator("us")) == 1, seen);

    f.chord(&[KEY_LEFTALT, KEY_LEFTSHIFT]);
    f.dispatch_until_explained(|_| locker.count(&indicator("es")) == 1, seen);

    f.chord(&[KEY_LEFTALT, KEY_LEFTSHIFT]);
    f.dispatch_until_explained(|_| locker.count(&indicator("us")) == 2, seen);
    assert_eq!(locker.count(INDICATOR_HIDDEN), 0, "{}", locker.seen());
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn the_layout_indicator_hides_with_one_layout_until_a_second_is_set() {
    let (mut f, locker) = locked_with_layouts(&["de"], answering);
    let seen = || locker.seen();
    f.dispatch_until_explained(|_| locker.count(INDICATOR_HIDDEN) == 1, seen);
    assert_eq!(locker.count(&indicator("de")), 0, "{}", locker.seen());

    // Showing it now says that the hidden one was Caminito's answer, not its absence.
    f.set_setting(KEYBOARD_XKB, "layouts", strings(&["de", "fr"]));
    f.dispatch_until_explained(|_| locker.count(&indicator("de")) == 1, seen);
}

#[test]
#[ignore = "needs CAMINITO_LOCKER, PAM_WRAPPER_MODULES, GTK and gtk-session-lock; tools/check runs it"]
fn the_layout_indicator_hides_when_caminito_cant_be_asked_and_the_dialog_still_opens() {
    // The names are held and not served, so no other fixture answers the Locker meanwhile.
    let (mut f, locker) = locked_with_layouts(&["us", "es"], Fixture::reserve_names);
    let seen = || locker.seen();
    f.dispatch_until_explained(|f| f.lock_state() == "locked", seen);
    f.dispatch_until_explained(|_| locker.count(INDICATOR_HIDDEN) == 1, seen);

    f.dispatch_until_explained(|_| locker.count(PROMPT) == 1, seen);
    tap(&mut f, KEY_A);
    f.dispatch_until_explained(|_| locker.count(DIALOG_SHOWS) == 1, seen);
    assert_eq!(locker.count(&indicator("us")), 0, "{}", locker.seen());
}
