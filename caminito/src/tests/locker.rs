//! The locker command (126): a lock request that isn't a locker's own spawns the
//! `locker-command`, which is restarted while the session is locked, and given up on after
//! too many failed starts. The test locker is a script that records its process and waits.
//! A fixture client that Caminito takes for that process asks for the lock in its place, as
//! the fixture's clients are all of the test's own process.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::Duration;

use super::fixture::{ClientId, Fixture, Logs};
use crate::helpers::RestartPolicy;
use crate::settings::Value;
use crate::settings::translate::{CAMINITO, LOCKDOWN};

/// What the test locker fills its lock surfaces with.
const LOCKER_COLOR: u32 = 0xff12_3456;

/// The session ID the give-up message names.
const SESSION_ID: &str = "c7";

/// Restarts quick enough for a test, with the supervisor's give-up rule.
fn quick_restarts() -> RestartPolicy {
    RestartPolicy {
        first_delay: Duration::from_millis(10),
        max_delay: Duration::from_millis(40),
        ..RestartPolicy::default()
    }
}

/// Test lockers in a scratch directory. Each run appends its process ID to `runs`.
struct Lockers {
    dir: tempfile::TempDir,
}

impl Lockers {
    fn new() -> Self {
        Lockers {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    /// A locker that waits for its stdin to close, as a locker runs until it unlocks.
    fn waiting(&self, name: &str) -> Vec<String> {
        self.write(name, "exec cat >/dev/null")
    }

    /// A locker that fails at once.
    fn failing(&self, name: &str) -> Vec<String> {
        self.write(name, "exit 1")
    }

    fn write(&self, name: &str, body: &str) -> Vec<String> {
        let path = self.dir.path().join(name);
        let script = format!("#!/bin/sh\necho $$ >>\"$(dirname \"$0\")/{name}.runs\"\n{body}\n");
        fs::write(&path, script).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        vec![path.to_string_lossy().into_owned()]
    }

    /// The process ID of each run of the locker `name`, oldest first. A line that a run is
    /// still writing has no newline yet, and isn't one.
    fn runs(&self, name: &str) -> Vec<u32> {
        let path: PathBuf = self.dir.path().join(format!("{name}.runs"));
        let text = fs::read_to_string(path).unwrap_or_default();
        let complete = text.rfind('\n').map_or("", |end| &text[..end]);
        complete.lines().map(|pid| pid.parse().unwrap()).collect()
    }
}

/// Caminito with one output and `locker-command` set to `command`, restarting it quickly.
fn fixture_with(command: Vec<String>, more: Vec<(&'static str, String, Value)>) -> Fixture {
    let mut settings = vec![(CAMINITO, "locker-command".into(), Value::StrList(command))];
    settings.extend(more);
    let mut f = Fixture::with_settings(settings);
    f.add_output((1920, 1080), 1.0);
    f.dispatch();
    f.state.caminito.locker.policy = quick_restarts();
    f.state.caminito.locker.session_id = Some(SESSION_ID.into());
    f
}

/// The dump's `locker`.
fn locker_dump(f: &Fixture) -> serde_json::Value {
    let dump: serde_json::Value =
        serde_json::from_str(&f.state.caminito.state_dump()).expect("the dump is JSON");
    dump["locker"].clone()
}

/// Waits for run `run` of the locker `name`, and connects a client as its process.
fn connect_as_run(f: &mut Fixture, lockers: &Lockers, name: &str, run: usize) -> ClientId {
    f.dispatch_until(|_| lockers.runs(name).len() > run);
    let pid = lockers.runs(name)[run];
    f.connect_as_process(pid)
}

/// Asks for the lock as `client`, with a lock surface on the output, and returns whether
/// the lock was accepted.
fn take_the_lock(f: &mut Fixture, client: ClientId) -> bool {
    f.client(client).bind_outputs();
    let lock = f.client(client).lock_session();
    f.roundtrip(client);
    if !f.client(client).session_lock(lock).locked {
        return false;
    }
    let surface = f.client(client).create_lock_surface(lock, 0);
    f.roundtrip(client);
    f.client(client).map_lock_surface(surface, LOCKER_COLOR);
    f.roundtrip(client);
    true
}

/// How many pixels of the first output's middle band, where the give-up message is, are
/// light, as the message's text is on the dark lock.
fn light_pixels_across_the_middle(f: &mut Fixture) -> usize {
    let output = f.state.caminito.outputs[0].clone();
    let width = output.current_mode().unwrap().size.w as usize;
    let pixels = f.state.backend.headless_mut().pixels(&output).unwrap();
    let middle = pixels.len() / width / 2;
    let band = &pixels[(middle - 40) * width..(middle + 40) * width];
    let light = |channel: u32| channel & 0xff >= 0x80;
    band.iter()
        .filter(|&&pixel| light(pixel >> 16) && light(pixel >> 8) && light(pixel))
        .count()
}

fn kill(pid: u32) {
    let pid = rustix::process::Pid::from_raw(pid as i32).unwrap();
    rustix::process::kill_process(pid, rustix::process::Signal::KILL).unwrap();
}

#[test]
fn a_lock_request_spawns_the_locker_and_its_lock_is_accepted() {
    let lockers = Lockers::new();
    let mut f = fixture_with(lockers.waiting("locker"), Vec::new());

    assert!(f.state.request_lock());

    assert_eq!(f.lock_state(), "pending");
    f.wait_for_pixel(100, 100, 0x000000);
    assert_eq!(light_pixels_across_the_middle(&mut f), 0);
    let locker = connect_as_run(&mut f, &lockers, "locker", 0);
    assert!(take_the_lock(&mut f, locker));
    assert_eq!(f.lock_state(), "locked");
    assert_eq!(locker_dump(&f)["state"], "running");
    f.wait_for_pixel(100, 100, LOCKER_COLOR & 0xff_ffff);
}

#[test]
fn another_client_is_refused_the_lock_the_spawned_locker_is_to_take() {
    let lockers = Lockers::new();
    let mut f = fixture_with(lockers.waiting("locker"), Vec::new());
    f.state.request_lock();
    f.dispatch_until(|_| !lockers.runs("locker").is_empty());
    let other = f.connect_as_process(lockers.runs("locker")[0] + 1);

    assert!(!take_the_lock(&mut f, other));

    assert_eq!(f.lock_state(), "pending");
}

#[test]
fn a_killed_locker_comes_back_and_takes_the_lock_over_with_the_session_locked_meanwhile() {
    let lockers = Lockers::new();
    let mut f = fixture_with(lockers.waiting("locker"), Vec::new());
    f.state.request_lock();
    let first = connect_as_run(&mut f, &lockers, "locker", 0);
    assert!(take_the_lock(&mut f, first));

    kill(lockers.runs("locker")[0]);
    f.disconnect(first);

    assert_eq!(f.lock_state(), "locker-lost");
    f.wait_for_pixel(100, 100, 0x000000);
    let second = connect_as_run(&mut f, &lockers, "locker", 1);
    assert!(take_the_lock(&mut f, second));
    assert_eq!(f.lock_state(), "locked");
    f.wait_for_pixel(100, 100, LOCKER_COLOR & 0xff_ffff);
}

#[test]
fn a_locker_that_unlocks_and_exits_isnt_started_again() {
    let lockers = Lockers::new();
    let mut f = fixture_with(lockers.waiting("locker"), Vec::new());
    f.state.request_lock();
    let locker = connect_as_run(&mut f, &lockers, "locker", 0);
    f.client(locker).bind_outputs();
    let lock = f.client(locker).lock_session();
    f.roundtrip(locker);

    f.client(locker).unlock_and_destroy(lock);
    f.roundtrip(locker);
    kill(lockers.runs("locker")[0]);
    f.disconnect(locker);

    assert_eq!(f.lock_state(), "unlocked");
    // Nothing is to happen: the wait is several of the restart's longest delays.
    f.dispatch_for(quick_restarts().max_delay * 5);
    assert_eq!(lockers.runs("locker").len(), 1);
    assert_eq!(locker_dump(&f)["state"], "idle");
}

#[test]
fn a_locker_that_always_fails_is_given_up_on_after_five_starts_with_the_session_locked() {
    let logs = Logs::capture();
    let lockers = Lockers::new();
    let mut f = fixture_with(lockers.failing("locker"), Vec::new());

    f.state.request_lock();
    f.dispatch_until(|f| locker_dump(f)["state"] == "gave-up");

    assert_eq!(lockers.runs("locker").len(), 5);
    assert_eq!(f.lock_state(), "pending");
    let message = locker_dump(&f)["message"].as_str().unwrap().to_owned();
    assert!(
        message.contains("loginctl unlock-session c7"),
        "the message is {message:?}"
    );
    assert!(logs.text().contains("locker"), "{}", logs.text());
    // Nothing is to happen: the wait is several of the restart's longest delays.
    f.dispatch_for(quick_restarts().max_delay * 5);
    assert_eq!(lockers.runs("locker").len(), 5);
    f.dispatch_until_mut(|f| light_pixels_across_the_middle(f) > 0);
}

#[test]
fn an_empty_locker_command_ends_in_the_give_up_state_with_the_session_locked() {
    let mut f = fixture_with(Vec::new(), Vec::new());

    f.state.request_lock();
    f.dispatch_until(|f| locker_dump(f)["state"] == "gave-up");

    assert_eq!(f.lock_state(), "pending");
}

#[test]
fn a_lock_request_is_refused_with_disable_lock_screen() {
    let logs = Logs::capture();
    let lockers = Lockers::new();
    let lockdown = (LOCKDOWN, "disable-lock-screen".into(), Value::Bool(true));
    let mut f = fixture_with(lockers.waiting("locker"), vec![lockdown]);

    assert!(!f.state.request_lock());

    assert_eq!(f.lock_state(), "unlocked");
    assert!(
        logs.text().contains("disable-lock-screen"),
        "{}",
        logs.text()
    );
    // Nothing is to happen: the wait is longer than a spawned script takes to record its run.
    f.dispatch_for(Duration::from_millis(200));
    assert!(lockers.runs("locker").is_empty());
}

#[test]
fn a_change_of_locker_command_is_used_by_the_next_lock() {
    let lockers = Lockers::new();
    let mut f = fixture_with(lockers.waiting("first"), Vec::new());
    let second = lockers.waiting("second");
    f.set_setting(CAMINITO, "locker-command", Value::StrList(second.clone()));
    f.dispatch_until(|f| f.state.caminito.settings.locker.command == second);

    f.state.request_lock();

    let locker = connect_as_run(&mut f, &lockers, "second", 0);
    assert!(take_the_lock(&mut f, locker));
    assert!(lockers.runs("first").is_empty());
}

#[test]
fn a_lock_request_while_the_locker_holds_the_lock_spawns_nothing_more() {
    let lockers = Lockers::new();
    let mut f = fixture_with(lockers.waiting("locker"), Vec::new());
    f.state.request_lock();
    let locker = connect_as_run(&mut f, &lockers, "locker", 0);
    assert!(take_the_lock(&mut f, locker));

    assert!(f.state.request_lock());

    // Nothing is to happen: the wait is longer than a spawned script takes to record its run.
    f.dispatch_for(Duration::from_millis(200));
    assert_eq!(lockers.runs("locker").len(), 1);
    assert_eq!(f.lock_state(), "locked");
}
