//! The mate-panel patch's side of Intellihide (CMN-224, ADR 0047) end to end: the patch's own
//! `wayland-intellihide.c`, in a probe that runs on GDK's Wayland backend with gtk-layer-shell,
//! asks for intellihide on its layer surface, and Caminito hides it under a maximised window
//! and shows it at the edge, as it does for the test client's (`intellihide.rs`). What this
//! shows is that the hand-written protocol of the patch is the one Caminito implements.
//!
//! It needs the probe built from the patch, named by `CAMINITO_INTELLIHIDE_PROBE`, so it's
//! ignored by default and `.scratch/checks/CMN-224/run.sh` builds the probe and runs it.

use std::io::{ErrorKind, Write};
use std::os::unix::net::UnixListener;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::Duration;

use super::fixture::{Fixture, KEY_F10, KEY_LEFTALT, Kind, WindowSpec};
use crate::settings::Value as Setting;
use crate::settings::translate::CAMINITO;

/// The probe in a scratch directory with its socket and its log.
struct Probe {
    dir: tempfile::TempDir,
    child: Child,
    stdin: ChildStdin,
}

impl Probe {
    fn start(f: &mut Fixture) -> Self {
        let program = std::env::var_os("CAMINITO_INTELLIHIDE_PROBE")
            .expect("CAMINITO_INTELLIHIDE_PROBE names the built intellihide-probe");
        let dir = tempfile::tempdir().unwrap();
        let listener = UnixListener::bind(dir.path().join("wayland-test")).unwrap();
        listener.set_nonblocking(true).unwrap();
        let log = std::fs::File::create(dir.path().join("log")).unwrap();
        let mut child = Command::new(program)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", dir.path())
            .env("XDG_RUNTIME_DIR", dir.path())
            .env("WAYLAND_DISPLAY", "wayland-test")
            .env("GDK_BACKEND", "wayland")
            .env("GTK_THEME", "Adwaita")
            .env("NO_AT_BRIDGE", "1")
            .env("GSETTINGS_BACKEND", "memory")
            .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent")
            .stdin(Stdio::piped())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .unwrap();
        let pid = child.id();
        let stdin = child.stdin.take().unwrap();
        f.dispatch_until_mut(|f| match listener.accept() {
            Ok((stream, _)) => {
                f.insert_stream(stream, Some(pid));
                true
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => false,
            Err(err) => panic!("can't accept the probe's connection: {err}"),
        });
        let mut probe = Probe { dir, child, stdin };
        probe.wait_for_line(f, "ready");
        probe
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("log")).unwrap_or_default()
    }

    fn wait_for_line(&mut self, f: &mut Fixture, line: &str) {
        let path = self.dir.path().join("log");
        f.dispatch_until_explained(
            |_| {
                let log = std::fs::read_to_string(&path).unwrap_or_default();
                log.lines().any(|l| l.starts_with(line))
            },
            || std::fs::read_to_string(&path).unwrap_or_default(),
        );
    }

    fn send(&mut self, line: &str) {
        writeln!(self.stdin, "{line}").unwrap();
        self.stdin.flush().unwrap();
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// What the dump says of the probe's panel: `None` while it doesn't ask for intellihide,
/// otherwise whether it is hidden.
fn hidden(f: &Fixture) -> Option<bool> {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let layers = dump["outputs"][0]["layer_surfaces"].as_array().unwrap();
    let panel = layers.iter().find(|l| l["namespace"] == "panel")?;
    panel["intellihide"]["hidden"].as_bool()
}

#[test]
#[ignore = "needs the probe built from the mate-panel patch, CAMINITO_INTELLIHIDE_PROBE"]
fn the_patched_panels_request_is_understood_and_ending_it_shows_the_panel() {
    let mut f = Fixture::with_settings(vec![(CAMINITO, "popup-slide".into(), Setting::Bool(true))]);
    f.add_output((1920, 1080), 1.0);
    let id = f.connect(Kind::Xdg);
    f.stop_clock();
    let window = f.map(id, WindowSpec::plain((400, 300)));
    f.pointer_move(960., 500.);
    let mut probe = Probe::start(&mut f);
    assert!(
        probe.log().contains("ready intellihide=yes"),
        "the probe found the global:\n{}",
        probe.log()
    );
    f.dispatch_until_mut(|f| {
        f.dispatch();
        let dump = f.state.caminito.state_dump();
        dump.contains("\"namespace\": \"panel\"") || dump.contains("\"namespace\":\"panel\"")
    });

    // It has not asked yet: an ordinary panel, under a window or not.
    f.chord(&[KEY_LEFTALT, KEY_F10]);
    f.draw_at_asked_size(window, (1920, 1080));
    f.advance_clock(Duration::from_millis(200));
    assert_eq!(hidden(&f), None);

    probe.send("ask");
    probe.wait_for_line(&mut f, "asked");
    f.dispatch_until(|f| hidden(f) == Some(true));
    f.advance_clock(Duration::from_millis(200));
    f.wait_for_pixel(960, 1066, 0x00ff00);

    // The edge reveals it.
    f.pointer_move(960., 1079.);
    f.advance_clock(Duration::from_millis(250));
    f.advance_clock(Duration::from_millis(200));
    assert_eq!(hidden(&f), Some(false));
    f.wait_for_pixel(960, 1066, 0x0000ff);

    // The panel leaves, and ending the request leaves it where it is.
    f.pointer_move(960., 500.);
    f.advance_clock(Duration::from_millis(200));
    assert_eq!(hidden(&f), Some(true));
    probe.send("stop");
    probe.wait_for_line(&mut f, "stopped");
    f.dispatch_until(|f| hidden(f).is_none());
    f.advance_clock(Duration::from_millis(200));
    f.wait_for_pixel(960, 1066, 0x0000ff);
}
