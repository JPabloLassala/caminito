//! `camino --session` (08, 61) on the test binary's private bus, with shell scripts in place
//! of mate-session, of the palette helper and of a D-Bus-activated service, and a stand-in
//! for systemd's user manager. No test maps a window: the Session's start and end don't
//! depend on them.
//!
//! The activation environment and `org.freedesktop.systemd1` are the bus's, not a
//! connection's, so every test here takes Camino's well-known names, which only one fixture
//! has at a time. The tests of a bus that stops (74) each run a bus of their own instead.

use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use camino_test_client::dbus::session;
use zbus::blocking::Connection;
use zbus::blocking::fdo::DBusProxy;

use super::fixture::{Bus, Fixture, Logs};
use crate::decoration::palette::PaletteSource;
use crate::helpers::{HelperCommand, RestartPolicy, StderrTarget};
use crate::session::SessionCommand;
use crate::settings::Value;
use crate::settings::translate::MOUSE;

const SYSTEMD: &str = "org.freedesktop.systemd1";

/// The socket name the tests' Sessions export. No test client connects by it.
const SOCKET: &str = "wayland-camino-test";

/// Every variable the Session exports whatever the machine (08). `XDG_CURRENT_DESKTOP` and
/// `XCURSOR_THEME` depend on it.
const EXPORTED: [&str; 6] = [
    "WAYLAND_DISPLAY",
    "DISPLAY",
    "XDG_SESSION_TYPE",
    "MATE_WM_NAME",
    "MATE_WM_KEYBINDINGS",
    "XCURSOR_SIZE",
];

/// The lines that wait until the script is signalled. A shell runs a trap only between
/// commands, so the wait is many short ones.
const WAIT: &str = "while :; do sleep 0.05; done";

/// The lines that wait until the test writes the file `name`.
fn wait_for(name: &str) -> String {
    format!("while [ ! -e \"$dir/{name}\" ]; do sleep 0.05; done")
}

/// Shell scripts in a scratch directory, and the files they leave there. Each starts with
/// `dir`, the directory, set.
struct Scripts {
    dir: tempfile::TempDir,
}

impl Scripts {
    fn new() -> Self {
        Scripts {
            dir: tempfile::tempdir().unwrap(),
        }
    }

    /// Writes the script `name`, and returns its path.
    fn write(&self, name: &str, body: &str) -> PathBuf {
        let path = self.path(name);
        fs::write(&path, format!("#!/bin/sh\ndir=$(dirname \"$0\")\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// A stand-in for mate-session that runs `body`.
    fn session_command(&self, body: &str) -> SessionCommand {
        SessionCommand::parse(self.write("session-command", body).as_os_str()).unwrap()
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    fn exists(&self, name: &str) -> bool {
        self.path(name).exists()
    }

    fn read(&self, name: &str) -> String {
        fs::read_to_string(self.path(name)).unwrap()
    }

    /// The environment a script wrote to `name` with `env`.
    fn environment(&self, name: &str) -> BTreeMap<String, String> {
        self.read(name)
            .lines()
            .filter_map(|line| line.split_once('='))
            .map(|(name, value)| (name.to_owned(), value.to_owned()))
            .collect()
    }
}

/// Writes the script's environment to `name`, which exists only once it's whole.
fn write_environment(name: &str) -> String {
    format!("env >\"$dir/{name}.partial\" && mv \"$dir/{name}.partial\" \"$dir/{name}\"")
}

/// Camino on the tests' bus with its names, as a Session has them (62), and with no
/// `org.freedesktop.systemd1` left on the bus by the test before.
fn fixture() -> Fixture {
    fixture_with_settings(Vec::new())
}

fn fixture_with_settings(settings: Vec<(&'static str, String, Value)>) -> Fixture {
    let mut f = Fixture::with_settings(settings);
    f.start_dbus_with_names();
    let bus = session().unwrap();
    let bus = DBusProxy::new(&bus).unwrap();
    // The bus frees a name some time after its owner's connection closes.
    f.dispatch_until(|_| !bus.name_has_owner(SYSTEMD.try_into().unwrap()).unwrap());
    f
}

/// Starts the Session on `command` and Xwayland, and returns Xwayland's display number.
fn start(f: &mut Fixture, command: SessionCommand) -> u32 {
    f.state.camino.start_session(command, SOCKET.into());
    f.start_xwayland()
}

/// A stand-in for systemd's user manager on the tests' bus. For each call it keeps the
/// method, its argument, and whether `started` existed then.
struct Systemd {
    calls: Calls,
    connection: Connection,
}

impl Drop for Systemd {
    /// A call it never answers keeps its connection open, and the name with it.
    fn drop(&mut self) {
        let _ = self.connection.release_name(SYSTEMD);
    }
}

type Calls = Arc<Mutex<Vec<(&'static str, Vec<String>, bool)>>>;

struct Manager {
    calls: Calls,
    started: PathBuf,
    /// How many more calls of `UnsetEnvironment` are never answered.
    unanswered: AtomicU32,
}

impl Manager {
    fn called(&self, method: &'static str, mut argument: Vec<String>) {
        argument.sort();
        let started = self.started.exists();
        self.calls.lock().unwrap().push((method, argument, started));
    }
}

#[zbus::interface(name = "org.freedesktop.systemd1.Manager")]
impl Manager {
    fn set_environment(&self, assignments: Vec<String>) {
        self.called("SetEnvironment", assignments);
    }

    async fn unset_environment(&self, names: Vec<String>) {
        self.called("UnsetEnvironment", names);
        let unanswered = |left: u32| left.checked_sub(1);
        if self
            .unanswered
            .try_update(Ordering::Relaxed, Ordering::Relaxed, unanswered)
            .is_ok()
        {
            std::future::pending::<()>().await;
        }
    }
}

impl Systemd {
    fn new(started: PathBuf) -> Self {
        Self::with(None, started, 0)
    }

    /// One whose `UnsetEnvironment` never answers, as a systemd that's shutting down.
    fn that_hangs(started: PathBuf) -> Self {
        Self::with(None, started, u32::MAX)
    }

    /// One that never answers its first `UnsetEnvironment`, and answers the ones after.
    fn that_hangs_once(started: PathBuf) -> Self {
        Self::with(None, started, 1)
    }

    /// One on `bus`, a test's own.
    fn on(bus: &Bus, started: PathBuf) -> Self {
        Self::with(Some(&bus.address), started, 0)
    }

    /// On the bus at `address`, or on the tests' bus, with `unanswered` calls of
    /// `UnsetEnvironment` that it never answers.
    fn with(address: Option<&str>, started: PathBuf, unanswered: u32) -> Self {
        let calls = Calls::default();
        let manager = Manager {
            calls: calls.clone(),
            started,
            unanswered: unanswered.into(),
        };
        let builder = match address {
            Some(address) => zbus::blocking::connection::Builder::address(address),
            None => zbus::blocking::connection::Builder::session(),
        };
        let connection = builder
            .unwrap()
            .name(SYSTEMD)
            .unwrap()
            .serve_at("/org/freedesktop/systemd1", manager)
            .unwrap()
            .build()
            .unwrap();
        Systemd { calls, connection }
    }

    fn calls(&self) -> Vec<(&'static str, Vec<String>, bool)> {
        self.calls.lock().unwrap().clone()
    }
}

/// A Session whose command has started, on a bus of the test's own with a stand-in for
/// systemd that has the environment. The command exits once `exit` is written.
fn session_on_its_own_bus(scripts: &Scripts) -> (Fixture, Bus, Systemd) {
    let bus = Bus::start(&scripts.path("bus"));
    let mut f = Fixture::with_settings(Vec::new());
    f.start_dbus_at(&bus.address);
    let systemd = Systemd::on(&bus, scripts.path("started"));
    let command = scripts.session_command(&format!("touch \"$dir/started\"\n{}", wait_for("exit")));
    start(&mut f, command);
    f.dispatch_until(|_| scripts.exists("started"));
    (f, bus, systemd)
}

/// A service file the tests' bus activates, in the directory for transient services, which
/// is the one the bus reads that a test can write to. It's removed when dropped.
struct ActivatedService {
    name: String,
    file: PathBuf,
}

impl ActivatedService {
    /// A service the bus starts by running `program`.
    fn new(program: &Path) -> Self {
        let name = format!("org.mate.Camino.Test.Activated{}", std::process::id());
        // The bus's own search path (`<standard_session_servicedirs/>`), first match first.
        let services = match std::env::var_os("XDG_RUNTIME_DIR") {
            Some(runtime) => PathBuf::from(runtime).join("dbus-1/services"),
            None => std::env::var_os("XDG_DATA_HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    PathBuf::from(std::env::var_os("HOME").unwrap()).join(".local/share")
                })
                .join("dbus-1/services"),
        };
        fs::create_dir_all(&services).unwrap();
        let file = services.join(format!("{name}.service"));
        let service = format!("[D-BUS Service]\nName={name}\nExec={}\n", program.display());
        fs::write(&file, service).unwrap();
        let bus = session().unwrap();
        DBusProxy::new(&bus).unwrap().reload_config().unwrap();
        ActivatedService { name, file }
    }
}

impl Drop for ActivatedService {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.file);
    }
}

#[test]
fn the_command_starts_once_xwayland_is_ready_with_the_names_owned_and_the_environment() {
    let theme = Value::Str("mate-black".into());
    let mut f = fixture_with_settings(vec![(MOUSE, "cursor-theme".into(), theme)]);
    let scripts = Scripts::new();
    let command = scripts.session_command(&format!(
        "dbus-send --session --print-reply --dest=org.freedesktop.DBus /org/freedesktop/DBus \
         org.freedesktop.DBus.NameHasOwner string:org.mate.ScreenSaver >\"$dir/owned\" 2>&1\n\
         {}\n{WAIT}",
        write_environment("environment")
    ));

    f.state.camino.start_session(command, SOCKET.into());
    // Nothing is to happen: the names are owned, and only Xwayland is missing. A command
    // that started would have written its file well within this.
    f.dispatch_for(Duration::from_millis(300));
    assert!(!scripts.exists("owned"), "the command waits for Xwayland");

    let display = f.start_xwayland();
    f.dispatch_until(|_| scripts.exists("environment"));

    assert!(scripts.read("owned").contains("boolean true"));
    let environment = scripts.environment("environment");
    let desktop = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or("MATE".into());
    let display = format!(":{display}");
    let expected = [
        ("WAYLAND_DISPLAY", SOCKET),
        ("DISPLAY", &display),
        ("XDG_SESSION_TYPE", "wayland"),
        ("XDG_CURRENT_DESKTOP", &desktop),
        ("MATE_WM_NAME", "Camino"),
        ("MATE_WM_KEYBINDINGS", "Camino"),
        ("XCURSOR_THEME", "mate-black"),
        ("XCURSOR_SIZE", "24"),
        ("GDK_BACKEND", "x11"),
    ];
    for (name, value) in expected {
        assert_eq!(
            environment.get(name).map(String::as_str),
            Some(value),
            "{name}"
        );
    }
    // Where camino-displays's desktop entry takes the Display capplet's place (84).
    let data_dirs = &environment["XDG_DATA_DIRS"];
    assert!(
        data_dirs.starts_with("/usr/share/camino/xdg:"),
        "{data_dirs}"
    );
}

#[test]
fn the_activation_environment_has_the_variables_before_the_command_starts() {
    let mut f = fixture();
    let scripts = Scripts::new();
    // It never takes its name, so the bus answers an activation that works only when the
    // request times out, and the command doesn't wait for that. One whose service file the
    // bus doesn't find is answered at once, so the command keeps the answer for the test's
    // failure (77). Its timeout is shorter than the wait, so it doesn't outlive the test.
    let program = scripts.write("service", &write_environment("activated"));
    let service = ActivatedService::new(&program);
    let command = scripts.session_command(&format!(
        "dbus-send --session --print-reply --reply-timeout=4000 --dest=org.freedesktop.DBus \
         /org/freedesktop/DBus \
         org.freedesktop.DBus.StartServiceByName string:{} uint32:0 \
         >\"$dir/answer\" 2>&1 &\n{WAIT}",
        service.name
    ));

    let display = start(&mut f, command);
    f.dispatch_until_explained(
        |_| scripts.exists("activated"),
        || {
            let answer = match fs::read_to_string(scripts.path("answer")) {
                Ok(answer) if !answer.is_empty() => answer,
                Ok(_) => "no answer yet: the bus is activating it, or it started and failed".into(),
                Err(_) => "nothing: the command hasn't asked".into(),
            };
            format!(
                "the activation with {} answered: {}",
                service.file.display(),
                answer.trim_end()
            )
        },
    );

    let activated = scripts.environment("activated");
    for name in EXPORTED {
        assert!(activated.contains_key(name), "{name} in {activated:?}");
    }
    assert_eq!(activated["WAYLAND_DISPLAY"], SOCKET);
    assert_eq!(activated["DISPLAY"], format!(":{display}"));
}

#[test]
fn without_systemd_on_the_bus_its_export_is_skipped_and_logged_once() {
    let logs = Logs::capture();
    let mut f = fixture();
    let scripts = Scripts::new();
    let command = scripts.session_command(&format!("touch \"$dir/started\"\n{WAIT}"));

    start(&mut f, command);
    f.dispatch_until(|_| scripts.exists("started"));

    let logs = logs.text();
    let skipped = logs.matches("org.freedesktop.systemd1 isn't on the bus");
    assert_eq!(skipped.count(), 1, "{logs}");
}

#[test]
fn systemd_is_given_the_environment_before_the_command_starts_and_forgets_it_on_exit() {
    let mut f = fixture();
    let scripts = Scripts::new();
    let systemd = Systemd::new(scripts.path("started"));
    let command = scripts.session_command("touch \"$dir/started\"");

    let display = start(&mut f, command);
    f.dispatch_until(|f| f.state.camino.session_exit().is_some());

    let calls = systemd.calls();
    let [(set, assignments, false), (unset, names, true)] = &calls[..] else {
        panic!("one call before the command and one after it: {calls:?}");
    };
    assert_eq!((*set, *unset), ("SetEnvironment", "UnsetEnvironment"));
    for name in EXPORTED {
        let assigned = assignments
            .iter()
            .any(|a| a.starts_with(&format!("{name}=")));
        assert!(assigned, "{name} in {assignments:?}");
    }
    assert!(assignments.contains(&format!("DISPLAY=:{display}")));
    assert!(!assignments.iter().any(|a| a.starts_with("GDK_BACKEND=")));
    // systemd outlives the Session, and the next login may be X11 MATE's (84).
    assert!(!assignments.iter().any(|a| a.starts_with("XDG_DATA_DIRS=")));
    assert_eq!(
        names,
        &[
            "DISPLAY",
            "MATE_WM_KEYBINDINGS",
            "MATE_WM_NAME",
            "WAYLAND_DISPLAY",
            "XCURSOR_SIZE",
            "XCURSOR_THEME"
        ]
    );
}

#[test]
fn the_commands_status_ends_the_session_once_xwayland_and_the_palette_helper_are_stopped() {
    let mut f = fixture();
    let scripts = Scripts::new();
    let helper = scripts.write(
        "helper",
        &format!(
            "trap 'touch \"$dir/helper-stopped\"; exit 0' TERM\ntouch \"$dir/helper\"\n{WAIT}"
        ),
    );
    let camino = &mut f.state.camino;
    camino.palette = PaletteSource::new(
        camino.loop_handle.clone(),
        HelperCommand::new("fake-helper", helper),
        RestartPolicy::default(),
        StderrTarget::Prefixed,
        None,
        None,
    );
    camino.palette.start();
    f.dispatch_until(|_| scripts.exists("helper"));
    // A restart, which an Xwayland that goes gets (42), would come this soon.
    let restart = Duration::from_millis(100);
    f.state.camino.xwayland_restart.policy.first_delay = restart;
    let command = scripts.session_command("exit 3");

    let display = start(&mut f, command);
    f.dispatch_until(|f| f.state.camino.session_exit().is_some());

    assert_eq!(f.state.camino.session_exit(), Some(3));
    // Both were told to stop before the Session was over, and Camino doesn't wait for them.
    f.dispatch_until(|f| scripts.exists("helper-stopped") && !f.xwayland_runs(display));
    f.dispatch_until(|f| f.state.camino.xwayland.is_none());
    // Nothing is to happen: Xwayland stays down. The wait is three of its restart's delays.
    // Another test's Xwayland may take the display meanwhile, so only Camino is asked.
    f.dispatch_for(restart * 3);
    assert!(f.state.camino.xwayland.is_none());
}

#[test]
fn a_stop_reaches_the_command_as_sigterm_and_the_session_ends_with_its_status() {
    let mut f = fixture();
    let scripts = Scripts::new();
    let command = scripts.session_command(&format!(
        "trap 'touch \"$dir/stopped\"; exit 7' TERM\ntouch \"$dir/started\"\n{WAIT}"
    ));
    start(&mut f, command);
    f.dispatch_until(|_| scripts.exists("started"));

    // What SIGTERM or SIGINT to Camino runs.
    f.state.camino.quit();
    f.dispatch_until(|f| f.state.camino.session_exit().is_some());

    assert!(scripts.exists("stopped"));
    assert_eq!(f.state.camino.session_exit(), Some(7));
}

#[test]
fn a_command_that_ignores_the_stop_is_given_up_on_after_the_wait() {
    let logs = Logs::capture();
    let mut f = fixture();
    let scripts = Scripts::new();
    let command = scripts.session_command(&format!(
        "trap '' TERM\necho $$ >\"$dir/pid.partial\" && mv \"$dir/pid.partial\" \"$dir/pid\"\n{WAIT}"
    ));
    start(&mut f, command);
    f.dispatch_until(|_| scripts.exists("pid"));
    // The 5 s of a real Session would be most of the test run.
    let wait = Duration::from_millis(400);
    f.state.camino.set_session_wait(wait);

    let stopped = Instant::now();
    f.state.camino.quit();
    // Nothing is to happen for the whole wait but its last part, which a slow machine may
    // have used up by here.
    f.dispatch_for(wait / 2);
    assert_eq!(f.state.camino.session_exit(), None);
    f.dispatch_until(|f| f.state.camino.session_exit().is_some());

    assert!(stopped.elapsed() >= wait);
    assert_eq!(f.state.camino.session_exit(), Some(1));
    let pid = scripts.read("pid");
    assert!(
        !Path::new("/proc").join(pid.trim()).exists(),
        "it was killed"
    );
    let logs = logs.text();
    assert!(logs.contains("the session command didn't exit"), "{logs}");
}

#[test]
fn a_systemd_that_doesnt_answer_the_unset_is_given_up_on_after_the_wait() {
    let logs = Logs::capture();
    let mut f = fixture();
    let scripts = Scripts::new();
    let systemd = Systemd::that_hangs(scripts.path("started"));
    let command = scripts.session_command(&format!("touch \"$dir/started\"\n{WAIT}"));
    start(&mut f, command);
    f.dispatch_until(|_| scripts.exists("started"));
    f.state.camino.set_session_wait(Duration::from_millis(400));

    f.state.camino.quit();
    f.dispatch_until(|f| f.state.camino.session_exit().is_some());

    let calls: Vec<&str> = systemd.calls().iter().map(|call| call.0).collect();
    assert_eq!(calls[..2], ["SetEnvironment", "UnsetEnvironment"]);
    let logs = logs.text();
    assert!(
        logs.contains("systemd didn't unset the environment"),
        "{logs}"
    );
}

#[test]
fn a_stop_during_the_export_spawns_no_command_and_systemd_still_forgets() {
    let mut f = fixture();
    let scripts = Scripts::new();
    let systemd = Systemd::new(scripts.path("started"));
    let command = scripts.session_command("touch \"$dir/started\"");
    start(&mut f, command);

    // Xwayland is ready, so the refresh pass has sent the export, and no dispatch since
    // has read the bus's answer.
    f.state.camino.quit();
    f.dispatch_until(|f| f.state.camino.session_exit().is_some());

    assert_eq!(f.state.camino.session_exit(), Some(1));
    assert!(!scripts.exists("started"));
    let calls: Vec<&str> = systemd.calls().iter().map(|call| call.0).collect();
    assert_eq!(calls, ["SetEnvironment", "UnsetEnvironment"]);
}

#[test]
fn a_systemd_that_doesnt_answer_the_unset_is_asked_again() {
    let logs = Logs::capture();
    let mut f = fixture();
    let scripts = Scripts::new();
    let systemd = Systemd::that_hangs_once(scripts.path("started"));
    let command = scripts.session_command("touch \"$dir/started\"");

    start(&mut f, command);
    f.dispatch_until(|f| f.state.camino.session_exit().is_some());

    let calls: Vec<&str> = systemd.calls().iter().map(|call| call.0).collect();
    assert_eq!(
        calls,
        ["SetEnvironment", "UnsetEnvironment", "UnsetEnvironment"]
    );
    let logs = logs.text();
    assert!(!logs.contains("systemd didn't unset"), "{logs}");
}

#[test]
fn the_systemd_on_the_bus_that_replaces_one_that_stops_with_the_command_forgets() {
    let logs = Logs::capture();
    let scripts = Scripts::new();
    let (mut f, bus, first) = session_on_its_own_bus(&scripts);

    // mate-session restarts the user's bus as it exits (74). No dispatch comes between the
    // two here, so Camino finds its connection closed as it sees the command gone.
    bus.stop();
    fs::write(scripts.path("exit"), "").unwrap();
    let bus = Bus::start(&scripts.path("bus"));
    let systemd = Systemd::on(&bus, scripts.path("started"));
    f.dispatch_until(|f| f.state.camino.session_exit().is_some());

    assert_eq!(f.state.camino.session_exit(), Some(0));
    let asked: Vec<&str> = first.calls().iter().map(|call| call.0).collect();
    assert_eq!(asked, ["SetEnvironment"]);
    let names = [
        "DISPLAY",
        "MATE_WM_KEYBINDINGS",
        "MATE_WM_NAME",
        "WAYLAND_DISPLAY",
        "XCURSOR_SIZE",
        "XCURSOR_THEME",
    ];
    let names = names.map(String::from).to_vec();
    // A machine slow enough to leave one request unanswered for its share of the wait
    // makes a second.
    let calls = systemd.calls();
    assert_eq!(calls.first(), Some(&("UnsetEnvironment", names, true)));
    assert!(calls.iter().all(|call| call == &calls[0]), "{calls:?}");
    let logs = logs.text();
    assert!(!logs.contains("systemd didn't unset"), "{logs}");
}

#[test]
fn a_bus_that_stops_with_the_command_and_isnt_replaced_is_given_up_on_after_the_wait() {
    let logs = Logs::capture();
    let scripts = Scripts::new();
    let (mut f, bus, _systemd) = session_on_its_own_bus(&scripts);
    let wait = Duration::from_millis(400);
    f.state.camino.set_session_wait(wait);

    bus.stop();
    fs::write(scripts.path("exit"), "").unwrap();
    let stopped = Instant::now();
    f.dispatch_until(|f| f.state.camino.session_exit().is_some());

    assert!(stopped.elapsed() >= wait);
    // Far less than the 5 s of a real Session: nothing but the wait holds Camino.
    assert!(stopped.elapsed() < wait * 5);
    assert_eq!(f.state.camino.session_exit(), Some(0));
    let logs = logs.text();
    assert!(
        logs.contains("systemd didn't unset the environment"),
        "{logs}"
    );
}
