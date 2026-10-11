//! A window that doesn't close (168): the ping that goes with a request to close, Marco's
//! "not responding" dialog five seconds after a ping with no answer, and Force Quit. Time
//! is the fixture's driven clock, and the dialog is 167's stand-in for zenity, whose status
//! is the button pressed. The programs killed are `sleep`s of the test's own.

use std::os::unix::process::ExitStatusExt;
use std::path::Path;
use std::process::{Child, Command};
use std::time::Duration;

use smithay::utils::Point;

use super::dialogs::{Press, Zenity};
use super::fixture::{
    ClientId, ClientWindow, Fixture, KEY_F4, KEY_LEFTALT, Kind, Logs, WindowSpec, on_both_kinds,
};

/// Marco's `PING_TIMEOUT_DELAY`.
const TIMEOUT: Duration = Duration::from_secs(5);

/// A program of the test's own for Force Quit to kill, killed with the test otherwise.
struct Program(Child);

impl Program {
    fn start() -> Self {
        Self(Command::new("sleep").arg("1000").spawn().unwrap())
    }

    fn pid(&self) -> u32 {
        self.0.id()
    }

    /// The signal that ended it, if it has ended.
    fn ended_by(&mut self) -> Option<i32> {
        self.0.try_wait().unwrap().map(|status| {
            status
                .signal()
                .expect("nothing but a signal ends the program")
        })
    }

    /// Dispatches until the program has ended, and returns the signal that ended it.
    fn wait(&mut self, f: &mut Fixture) -> i32 {
        f.dispatch_until_mut(|_| self.0.try_wait().unwrap().is_some());
        self.ended_by().unwrap()
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn hostname() -> String {
    let uname = rustix::system::uname();
    uname.nodename().to_str().unwrap().to_owned()
}

/// A window titled `title` that takes pings, as every toolkit's does.
fn spec(title: &'static str) -> WindowSpec {
    WindowSpec {
        title: Some(title),
        ping: true,
        ..WindowSpec::default()
    }
}

/// Sets `WM_CLIENT_MACHINE`, which says where `_NET_WM_PID`'s process is.
fn set_machine(f: &mut Fixture, window: ClientWindow, machine: &str) {
    let (xid, machine) = (window.x11(), machine.to_owned());
    f.x11_call(window.client(), move |x| {
        x.set_string(xid, "WM_CLIENT_MACHINE", &machine)
    })
    .unwrap();
}

/// Maps a window of `kind` titled "Notes" whose client answers no ping, as a program that
/// hangs. It's of the process `pid`, as Caminito sees it: its Wayland client's, or its
/// `_NET_WM_PID` on this machine. With `None` an xdg window is of the test's own process,
/// and an X11 window says nothing.
fn hung_window(f: &mut Fixture, kind: Kind, pid: Option<u32>) -> ClientWindow {
    match (kind, pid) {
        (Kind::Xdg, pid) => {
            let client = match pid {
                Some(pid) => f.connect_as_process(pid),
                None => f.connect(Kind::Xdg),
            };
            f.client(client).hang();
            f.map(client, spec("Notes"))
        }
        // An X client answers a ping only when the test has it answer.
        (Kind::X11, pid) => {
            let client = f.connect(Kind::X11);
            let mut spec = spec("Notes");
            if let Some(pid) = pid {
                spec.cardinals = vec![("_NET_WM_PID", vec![pid])];
            }
            let window = f.map(client, spec);
            if pid.is_some() {
                set_machine(f, window, &hostname());
            }
            window
        }
    }
}

/// Has the window's client answer the pings it has had.
fn answer_pings(f: &mut Fixture, window: ClientWindow) {
    match window {
        ClientWindow::Xdg(client, _) => {
            f.client(client).respond_again();
            f.roundtrip(client);
        }
        ClientWindow::X11(client, xid) => {
            let answered = f.x11_call(client, move |x| x.answer_pings(xid)).unwrap();
            assert!(answered > 0, "the window was pinged");
            // Xwayland has passed the answers on by the time the client's sync returns.
            f.dispatch();
        }
    }
}

/// Alt+F4 on the focused window, which the newest window is.
fn ask_to_close(f: &mut Fixture) {
    f.chord(&[KEY_LEFTALT, KEY_F4]);
}

/// Whether the window's client has lost its connection, as Force Quit leaves it.
fn is_cut_off(f: &mut Fixture, client: ClientId) -> bool {
    f.x11_call(client, |x| x.sync()).is_err()
}

fn dialogs(f: &Fixture) -> usize {
    f.state.caminito.not_responding.dialogs()
}

/// Waits for a dialog to have ended with the button it was given, and for Caminito to have
/// acted on it.
fn wait_for_the_dialogs_end(f: &mut Fixture, zenity: &Zenity) {
    let ran = zenity.ran();
    f.dispatch_until(|f| ran.exists() && dialogs(f) == 0);
}

fn runs(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

/// Marco's dialog for a window titled `title`, as markup.
fn not_responding_dialog(title: &str) -> Vec<String> {
    let text = format!(
        "<big><b><tt>{title}</tt> is not responding.</b></big>\n\n\
         <i>You may choose to wait a short while for it to continue or force the \
         application to quit entirely.</i>"
    );
    [
        "--question",
        "--title",
        "Marco",
        "--text",
        &text,
        "--ok-label",
        "_Wait",
        "--cancel-label",
        "_Force Quit",
    ]
    .map(str::to_owned)
    .to_vec()
}

// The ping, and the dialog.

on_both_kinds! {
    fn a_window_whose_client_answers_no_ping_gets_marcos_dialog_five_seconds_after_the_close(
        kind: Kind
    ) {
        let mut f = Fixture::new();
        f.add_output((1920, 1080), 1.0);
        let zenity = Zenity::install_pressing(&mut f, Press::Ok);
        let window = hung_window(&mut f, kind, None);

        ask_to_close(&mut f);
        assert!(f.close_requested(window));
        // The test's own time passes too, which is far less than a second.
        f.advance_clock(TIMEOUT - Duration::from_secs(1));
        assert_eq!(dialogs(&f), 0);
        f.advance_clock(Duration::from_secs(1));

        assert_eq!(zenity.arguments(&mut f), not_responding_dialog("Notes"));
    }
}

on_both_kinds! {
    fn a_window_whose_client_answers_the_ping_gets_no_dialog(kind: Kind) {
        let mut f = Fixture::new();
        f.add_output((1920, 1080), 1.0);
        let zenity = Zenity::install_pressing(&mut f, Press::Ok);
        let window = hung_window(&mut f, kind, None);

        ask_to_close(&mut f);
        answer_pings(&mut f, window);
        f.advance_clock(TIMEOUT);

        // The dialog's process is spawned as the time passes, or it isn't.
        assert_eq!(dialogs(&f), 0);
        assert_eq!(zenity.starts(), 0);
    }
}

#[test]
fn an_xdg_client_is_pinged_when_its_window_is_asked_to_close_and_not_before() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    let window = hung_window(&mut f, Kind::Xdg, None);
    f.advance_clock(2 * TIMEOUT);
    assert_eq!(f.client(window.client()).unanswered_pings(), 0);

    ask_to_close(&mut f);
    f.roundtrip(window.client());

    assert_eq!(f.client(window.client()).unanswered_pings(), 1);
}

/// Marco takes a window with no `_NET_WM_PING` as one that answered
/// (`meta_display_ping_window`, `core/display.c:4441-4447`).
#[test]
fn an_x11_window_that_takes_no_ping_gets_no_dialog() {
    let (mut f, client) = Fixture::connected(Kind::X11);
    let zenity = Zenity::install_pressing(&mut f, Press::Ok);
    let window = f.map(client, WindowSpec::default());

    ask_to_close(&mut f);
    assert!(f.close_requested(window));
    f.advance_clock(TIMEOUT);

    assert_eq!(dialogs(&f), 0);
    assert_eq!(zenity.starts(), 0);
}

#[test]
fn the_title_in_the_dialog_is_escaped_for_its_markup() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    let zenity = Zenity::install_pressing(&mut f, Press::Ok);
    let client = f.connect(Kind::Xdg);
    f.client(client).hang();
    f.map(client, spec("a <b> & c"));

    ask_to_close(&mut f);
    f.advance_clock(TIMEOUT);

    assert_eq!(
        zenity.arguments(&mut f),
        not_responding_dialog("a &lt;b&gt; &amp; c")
    );
}

// Force Quit.

on_both_kinds! {
    fn force_quit_kills_the_windows_process(kind: Kind) {
        let mut f = Fixture::new();
        f.add_output((1920, 1080), 1.0);
        let zenity = Zenity::install_pressing(&mut f, Press::Cancel);
        let mut program = Program::start();
        hung_window(&mut f, kind, Some(program.pid()));

        ask_to_close(&mut f);
        f.advance_clock(TIMEOUT);

        assert_eq!(program.wait(&mut f), libc::SIGKILL);
        wait_for_the_dialogs_end(&mut f, &zenity);
    }
}

on_both_kinds! {
    fn wait_kills_nothing_and_the_next_close_asks_again(kind: Kind) {
        let mut f = Fixture::new();
        f.add_output((1920, 1080), 1.0);
        let zenity = Zenity::install_pressing(&mut f, Press::Ok);
        let mut program = Program::start();
        let window = hung_window(&mut f, kind, Some(program.pid()));

        ask_to_close(&mut f);
        f.advance_clock(TIMEOUT);
        wait_for_the_dialogs_end(&mut f, &zenity);
        assert_eq!(program.ended_by(), None);
        assert!(f.policy_id(window).is_some());
        // Nothing more comes of the ping that had no answer.
        f.advance_clock(TIMEOUT);
        assert_eq!(zenity.starts(), 1);

        ask_to_close(&mut f);
        f.advance_clock(TIMEOUT);

        f.dispatch_until(|_| zenity.starts() == 2);
    }
}

on_both_kinds! {
    /// Not as in Marco, where zenity's status for Escape and for its window closed is
    /// Force Quit's.
    fn escape_in_the_dialog_or_closing_it_is_wait(kind: Kind) {
        let mut f = Fixture::new();
        f.add_output((1920, 1080), 1.0);
        let zenity = Zenity::install_pressing(&mut f, Press::Escape);
        let mut program = Program::start();
        let window = hung_window(&mut f, kind, Some(program.pid()));

        ask_to_close(&mut f);
        f.advance_clock(TIMEOUT);
        wait_for_the_dialogs_end(&mut f, &zenity);

        assert_eq!(program.ended_by(), None);
        assert!(f.policy_id(window).is_some());
    }
}

/// Marco's `meta_window_kill` (`core/delete.c:162-200`) cuts every window's client off,
/// and signals only a process it knows.
#[test]
fn force_quit_cuts_off_an_x11_client_with_no_net_wm_pid() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    let zenity = Zenity::install_pressing(&mut f, Press::Cancel);
    let window = hung_window(&mut f, Kind::X11, None);

    ask_to_close(&mut f);
    f.advance_clock(TIMEOUT);
    wait_for_the_dialogs_end(&mut f, &zenity);

    f.dispatch_until(|f| f.policy_id(window).is_none());
    assert!(is_cut_off(&mut f, window.client()));
}

/// `_NET_WM_PID` is a process of the machine `WM_CLIENT_MACHINE` names, which another
/// machine's client, over `ssh -X`, isn't of this one.
#[test]
fn force_quit_signals_no_process_for_an_x11_client_of_another_machine() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    let zenity = Zenity::install_pressing(&mut f, Press::Cancel);
    let mut program = Program::start();
    let window = hung_window(&mut f, Kind::X11, Some(program.pid()));
    set_machine(&mut f, window, "another.example");

    ask_to_close(&mut f);
    f.advance_clock(TIMEOUT);
    wait_for_the_dialogs_end(&mut f, &zenity);

    f.dispatch_until(|f| f.policy_id(window).is_none());
    assert!(is_cut_off(&mut f, window.client()));
    assert_eq!(program.ended_by(), None);
}

/// The fixture's clients are of the test's process, as Caminito's own would be of
/// Caminito's: a Force Quit that signalled it would end the test.
#[test]
fn force_quit_never_kills_caminitos_own_process() {
    let logs = Logs::capture();
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    let zenity = Zenity::install_pressing(&mut f, Press::Cancel);
    let window = hung_window(&mut f, Kind::Xdg, None);

    ask_to_close(&mut f);
    f.advance_clock(TIMEOUT);
    wait_for_the_dialogs_end(&mut f, &zenity);

    assert!(f.policy_id(window).is_some());
    let logs = logs.text();
    assert!(
        logs.contains("can't force \"Notes\" to quit: its process is Caminito's own"),
        "{logs}"
    );
}

#[test]
fn force_quit_never_kills_xwayland() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    let zenity = Zenity::install_pressing(&mut f, Press::Cancel);
    let display = f.start_xwayland();
    let xwayland = crate::xwayland::process(display).unwrap();
    let pid = u32::try_from(xwayland.as_raw_nonzero().get()).unwrap();
    let window = hung_window(&mut f, Kind::X11, Some(pid));

    ask_to_close(&mut f);
    f.advance_clock(TIMEOUT);
    wait_for_the_dialogs_end(&mut f, &zenity);

    // Its client is cut off all the same.
    f.dispatch_until(|f| f.policy_id(window).is_none());
    assert!(f.xwayland_runs(display));
    assert!(f.state.caminito.xwayland_display().is_some());
}

// The dialog taken away.

on_both_kinds! {
    fn the_dialog_is_taken_away_when_the_window_closes(kind: Kind) {
        let mut f = Fixture::new();
        f.add_output((1920, 1080), 1.0);
        let zenity = Zenity::install_pressing(&mut f, Press::Cancel);
        zenity.hold();
        let mut program = Program::start();
        let window = hung_window(&mut f, kind, Some(program.pid()));
        ask_to_close(&mut f);
        f.advance_clock(TIMEOUT);
        f.dispatch_until(|_| zenity.starts() == 1);
        let dialog = zenity.processes()[0];

        match window {
            ClientWindow::Xdg(client, id) => f.client(client).destroy_window(id),
            ClientWindow::X11(client, xid) => {
                f.x11_call(client, move |x| x.destroy(xid)).unwrap();
            }
        }

        // Killed, and reaped.
        f.dispatch_until(|f| !runs(dialog) && dialogs(f) == 0);
        assert!(f.policy_id(window).is_none());
        assert_eq!(program.ended_by(), None);
    }
}

on_both_kinds! {
    fn the_dialog_is_taken_away_when_the_ping_is_answered(kind: Kind) {
        let mut f = Fixture::new();
        f.add_output((1920, 1080), 1.0);
        let zenity = Zenity::install_pressing(&mut f, Press::Cancel);
        zenity.hold();
        let mut program = Program::start();
        let window = hung_window(&mut f, kind, Some(program.pid()));
        ask_to_close(&mut f);
        f.advance_clock(TIMEOUT);
        f.dispatch_until(|_| zenity.starts() == 1);
        let dialog = zenity.processes()[0];

        answer_pings(&mut f, window);

        f.dispatch_until(|f| !runs(dialog) && dialogs(f) == 0);
        assert!(f.policy_id(window).is_some());
        assert_eq!(program.ended_by(), None);
    }
}

on_both_kinds! {
    fn a_close_while_the_dialog_shows_shows_no_other(kind: Kind) {
        let mut f = Fixture::new();
        f.add_output((1920, 1080), 1.0);
        let zenity = Zenity::install_pressing(&mut f, Press::Ok);
        zenity.hold();
        hung_window(&mut f, kind, None);
        ask_to_close(&mut f);
        f.advance_clock(TIMEOUT);
        f.dispatch_until(|_| zenity.starts() == 1);

        ask_to_close(&mut f);
        f.advance_clock(TIMEOUT);

        assert_eq!(dialogs(&f), 1);
        zenity.release();
        wait_for_the_dialogs_end(&mut f, &zenity);
        assert_eq!(zenity.starts(), 1);
    }
}

/// Marco's `meta_window_present_delete_dialog` (`core/delete.c:211-248`) activates the
/// dialog that shows. The dialog is a window of zenity's process.
#[test]
fn a_close_while_the_dialog_shows_brings_the_dialog_forward() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    let zenity = Zenity::install_pressing(&mut f, Press::Ok);
    zenity.hold();
    let window = hung_window(&mut f, Kind::Xdg, None);
    ask_to_close(&mut f);
    f.advance_clock(TIMEOUT);
    f.dispatch_until(|_| zenity.starts() == 1);
    // The stand-in has no window, so a client of the test's is its dialog, which the
    // user leaves for the window.
    let dialog_client = f.connect_as_process(zenity.processes()[0]);
    let dialog = f.map(dialog_client, WindowSpec::default());
    assert!(f.has_keyboard(dialog));
    let inside = f.geometry(window).loc + Point::new(10, 10);
    f.click(inside.x.into(), inside.y.into());
    assert!(f.has_keyboard(window));

    ask_to_close(&mut f);
    f.advance_clock(TIMEOUT);

    assert!(f.has_keyboard(dialog));
    zenity.release();
}

#[test]
fn zenity_missing_is_logged_and_nothing_shows() {
    let logs = Logs::capture();
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    // A `PATH` of one empty directory has no zenity.
    let empty = tempfile::tempdir().unwrap();
    f.state
        .caminito
        .environment
        .insert("PATH".into(), empty.path().into());
    hung_window(&mut f, Kind::Xdg, None);

    ask_to_close(&mut f);
    f.advance_clock(TIMEOUT);

    let logs = logs.text();
    assert!(
        logs.contains("can't show Marco's dialog for a window that isn't responding: zenity: "),
        "{logs}"
    );
    assert_eq!(dialogs(&f), 0);
}
