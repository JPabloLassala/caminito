//! logind (130): its `Lock` and `Unlock` signals for Caminito's session, the locked hint, and
//! the `sleep` delay inhibitor held until a pending lock is on screen. A stand-in logind
//! serves the manager and the session on a bus of the test's own, and keeps the read end of
//! each inhibitor's pipe, which hangs up once Caminito lets the inhibitor go, as logind's
//! FIFO does.

use std::os::fd::{AsFd, OwnedFd};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use zbus::fdo;
use zbus::zvariant::OwnedObjectPath;

use super::fixture::{Bus, Fixture, Logs};
use super::locker::{
    Lockers, SESSION_ID, connect_as_run, fixture_with, locker_dump, take_the_lock,
};

const LOGIN1: &str = "org.freedesktop.login1";
const MANAGER_PATH: &str = "/org/freedesktop/login1";
const MANAGER: &str = "org.freedesktop.login1.Manager";
const SESSION: &str = "org.freedesktop.login1.Session";

/// What the stand-in was asked, which the test reads.
#[derive(Default)]
struct Asked {
    /// Each `SetLockedHint`, in order.
    hints: Vec<bool>,
    /// Each inhibitor's `what`, `mode` and the read end of its pipe.
    inhibitors: Vec<(String, String, OwnedFd)>,
}

struct Manager {
    asked: Arc<Mutex<Asked>>,
    /// The session `GetSession` knows, if any.
    session: Option<&'static str>,
}

fn session_path(id: &str) -> String {
    format!("{MANAGER_PATH}/session/{id}")
}

#[zbus::interface(name = "org.freedesktop.login1.Manager")]
impl Manager {
    fn get_session(&self, id: String) -> fdo::Result<OwnedObjectPath> {
        match self.session {
            Some(session) if session == id => Ok(OwnedObjectPath::try_from(session_path(&id))
                .map_err(|err| fdo::Error::Failed(err.to_string()))?),
            _ => Err(fdo::Error::Failed(format!("no session '{id}' known"))),
        }
    }

    fn get_session_by_pid(&self, pid: u32) -> fdo::Result<OwnedObjectPath> {
        Err(fdo::Error::Failed(format!("PID {pid} has no session")))
    }

    fn inhibit(
        &self,
        what: String,
        _who: String,
        _why: String,
        mode: String,
    ) -> zbus::zvariant::OwnedFd {
        let (read, write) = rustix::pipe::pipe_with(rustix::pipe::PipeFlags::CLOEXEC).unwrap();
        self.asked
            .lock()
            .unwrap()
            .inhibitors
            .push((what, mode, read));
        write.into()
    }
}

struct Session {
    asked: Arc<Mutex<Asked>>,
}

#[zbus::interface(name = "org.freedesktop.login1.Session")]
impl Session {
    fn set_locked_hint(&self, locked: bool) {
        self.asked.lock().unwrap().hints.push(locked);
    }
}

/// A stand-in logind on `bus`, which knows the session `session`.
struct Login1 {
    asked: Arc<Mutex<Asked>>,
    connection: zbus::blocking::Connection,
}

impl Login1 {
    fn on(bus: &Bus, session: Option<&'static str>) -> Self {
        let asked = Arc::new(Mutex::new(Asked::default()));
        let manager = Manager {
            asked: asked.clone(),
            session,
        };
        let mut builder = zbus::blocking::connection::Builder::address(bus.address.as_str())
            .unwrap()
            .name(LOGIN1)
            .unwrap()
            .serve_at(MANAGER_PATH, manager)
            .unwrap();
        if let Some(session) = session {
            let path = session_path(session);
            let session = Session {
                asked: asked.clone(),
            };
            builder = builder.serve_at(path, session).unwrap();
        }
        let connection = builder.build().unwrap();
        Login1 { asked, connection }
    }

    fn lock(&self) {
        self.session_signal("Lock");
    }

    fn unlock(&self) {
        self.session_signal("Unlock");
    }

    fn session_signal(&self, name: &str) {
        self.connection
            .emit_signal(None::<()>, session_path(SESSION_ID), SESSION, name, &())
            .unwrap();
    }

    fn prepare_for_sleep(&self, start: bool) {
        self.connection
            .emit_signal(None::<()>, MANAGER_PATH, MANAGER, "PrepareForSleep", &start)
            .unwrap();
    }

    fn hints(&self) -> Vec<bool> {
        self.asked.lock().unwrap().hints.clone()
    }

    /// How many inhibitors Caminito was given.
    fn inhibitors_taken(&self) -> usize {
        self.asked.lock().unwrap().inhibitors.len()
    }

    /// Whether Caminito holds a `sleep` delay inhibitor: one whose pipe's write end is open.
    fn sleep_delay_held(&self) -> bool {
        let asked = self.asked.lock().unwrap();
        asked.inhibitors.iter().any(|(what, mode, read)| {
            assert_eq!((what.as_str(), mode.as_str()), ("sleep", "delay"));
            !hung_up(read)
        })
    }
}

/// Whether every write end of the pipe `read` reads from is closed.
fn hung_up(read: &OwnedFd) -> bool {
    use rustix::event::{PollFd, PollFlags, poll};
    let read = read.as_fd();
    let mut fds = [PollFd::new(&read, PollFlags::IN)];
    let zero = rustix::event::Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    poll(&mut fds, Some(&zero)).unwrap();
    fds[0].revents().contains(PollFlags::HUP)
}

/// Caminito with the locker `command`, on a bus of the test's own with a stand-in logind
/// that knows its session, once it holds the sleep delay.
fn with_logind(command: Vec<String>) -> (Fixture, Bus, Login1, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let bus = Bus::start(&dir.path().join("bus"));
    let login1 = Login1::on(&bus, Some(SESSION_ID));
    let mut f = fixture_with(command, Vec::new());
    f.start_logind_at(&bus.address);
    f.dispatch_until(|_| login1.sleep_delay_held());
    (f, bus, login1, dir)
}

fn process_is_gone(pid: u32) -> bool {
    !std::path::Path::new(&format!("/proc/{pid}")).exists()
}

#[test]
fn loginds_lock_locks_the_session_and_spawns_the_locker() {
    let lockers = Lockers::new();
    let (mut f, _bus, login1, _dir) = with_logind(lockers.waiting("locker"));

    login1.lock();

    f.dispatch_until(|f| f.lock_state() == "pending");
    let locker = connect_as_run(&mut f, &lockers, "locker", 0);
    assert!(take_the_lock(&mut f, locker));
    assert_eq!(f.lock_state(), "locked");
}

#[test]
fn loginds_unlock_unlocks_and_stops_the_locker() {
    let lockers = Lockers::new();
    let (mut f, _bus, login1, _dir) = with_logind(lockers.waiting("locker"));
    login1.lock();
    let locker = connect_as_run(&mut f, &lockers, "locker", 0);
    f.client(locker).bind_outputs();
    let lock = f.client(locker).lock_session();
    f.roundtrip(locker);
    assert!(f.client(locker).session_lock(lock).locked);

    login1.unlock();

    f.dispatch_until(|f| f.lock_state() == "unlocked");
    f.roundtrip(locker);
    assert!(f.client(locker).session_lock(lock).finished);
    let pid = lockers.runs("locker")[0];
    f.dispatch_until(|_| process_is_gone(pid));
    assert_eq!(locker_dump(&f)["state"], "idle");
}

#[test]
fn loginds_unlock_unlocks_from_the_give_up_state() {
    let lockers = Lockers::new();
    let (mut f, _bus, login1, _dir) = with_logind(lockers.failing("locker"));
    login1.lock();
    f.dispatch_until(|f| locker_dump(f)["state"] == "gave-up");

    login1.unlock();

    f.dispatch_until(|f| f.lock_state() == "unlocked");
    assert_eq!(locker_dump(&f)["state"], "idle");
    // The next lock gets a locker again.
    let runs = lockers.runs("locker").len();
    login1.lock();
    f.dispatch_until(|_| lockers.runs("locker").len() > runs);
}

#[test]
fn the_locked_hint_is_set_with_each_lock_and_unlock() {
    let lockers = Lockers::new();
    let (mut f, _bus, login1, _dir) = with_logind(lockers.waiting("locker"));

    login1.lock();
    f.dispatch_until(|_| login1.hints() == [true]);
    login1.unlock();
    f.dispatch_until(|_| login1.hints() == [true, false]);
    assert!(f.state.request_lock());
    f.dispatch_until(|_| login1.hints() == [true, false, true]);
}

#[test]
fn the_sleep_delay_is_released_once_the_pending_lock_is_drawn_and_taken_again_on_resume() {
    let lockers = Lockers::new();
    let (mut f, _bus, login1, _dir) = with_logind(lockers.waiting("locker"));
    login1.lock();
    let locker = connect_as_run(&mut f, &lockers, "locker", 0);
    // The locker holds the lock, with nothing drawn on it yet.
    f.client(locker).bind_outputs();
    let lock = f.client(locker).lock_session();
    f.roundtrip(locker);
    assert_eq!(f.lock_state(), "locked");

    login1.prepare_for_sleep(true);

    // Nothing is to happen until the locker draws: the wait is well short of the 2 s limit.
    f.dispatch_for(Duration::from_millis(500));
    assert!(login1.sleep_delay_held());
    let surface = f.client(locker).create_lock_surface(lock, 0);
    f.roundtrip(locker);
    f.client(locker).map_lock_surface(surface, 0xff12_3456);
    f.dispatch_until(|_| !login1.sleep_delay_held());

    login1.prepare_for_sleep(false);

    f.dispatch_until(|_| login1.sleep_delay_held());
    assert_eq!(login1.inhibitors_taken(), 2);
}

#[test]
fn the_sleep_delay_is_released_after_2_s_with_a_locker_that_never_draws() {
    let lockers = Lockers::new();
    let (mut f, _bus, login1, _dir) = with_logind(lockers.waiting("locker"));
    login1.lock();
    connect_as_run(&mut f, &lockers, "locker", 0);

    let asked = Instant::now();
    login1.prepare_for_sleep(true);

    f.dispatch_until(|_| !login1.sleep_delay_held());
    assert!(
        asked.elapsed() >= Duration::from_millis(1900),
        "{:?}",
        asked.elapsed()
    );
    assert_eq!(f.lock_state(), "pending");
}

#[test]
fn the_sleep_delay_is_released_at_once_while_unlocked() {
    let lockers = Lockers::new();
    let (mut f, _bus, login1, _dir) = with_logind(lockers.waiting("locker"));

    let asked = Instant::now();
    login1.prepare_for_sleep(true);

    f.dispatch_until(|_| !login1.sleep_delay_held());
    assert!(
        asked.elapsed() < Duration::from_secs(1),
        "{:?}",
        asked.elapsed()
    );
}

#[test]
fn without_a_logind_session_caminito_logs_once_and_locks_as_before() {
    let logs = Logs::capture();
    let lockers = Lockers::new();
    let dir = tempfile::tempdir().unwrap();
    let bus = Bus::start(&dir.path().join("bus"));
    let _login1 = Login1::on(&bus, None);
    let mut f = fixture_with(lockers.waiting("locker"), Vec::new());

    f.start_logind_at(&bus.address);

    f.dispatch_until(|_| logs.text().contains("logind"));
    assert!(f.state.request_lock());
    f.dispatch_until(|_| !lockers.runs("locker").is_empty());
    assert_eq!(f.lock_state(), "pending");
    let lines = logs.text();
    let logind_lines: Vec<&str> = lines.lines().filter(|l| l.contains("logind")).collect();
    assert_eq!(logind_lines.len(), 1, "{lines}");
}
