//! The Sound popup's wheel end to end (CMN-215, as CMN-210's spike measured it): the popup's
//! content, in `sd-wheel-probe`, runs on GDK's Wayland backend against Caminito and gets the
//! wheel over its output's slider through Caminito's own input path, as the fixture sends
//! it: a whole click, a click in eighths as the Wheel glide of ADR 0039 would send it (and a
//! hi-res wheel does), two clicks at once, and a touchpad's fingers, with and without Shift.
//! The plain wheel scrolls the popup and leaves the slider; Shift moves the slider by
//! volume-step a click, the way the wheel turns, and leaves the popup. A click's smooth delta
//! on this backend is 1.5, not X11's 1.0, which `.scratch/checks/CMN-215` checks on X11.
//!
//! It needs the built probe, named by `CAMINITO_SOUND_WHEEL_PROBE`, so it's ignored by
//! default and `tools/check` runs it after the C helpers' build.

use std::io::{ErrorKind, Write};
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::Duration;

use super::fixture::{Fixture, KEY_LEFTSHIFT};

/// One whole click, down.
const CLICK: f64 = 120.;

/// The probe in a scratch directory with its socket and its log.
struct Probe {
    dir: tempfile::TempDir,
    child: Child,
    stdin: ChildStdin,
    /// How much of the log the scenarios so far have read.
    read: usize,
}

impl Probe {
    fn start(f: &mut Fixture) -> Self {
        let program = std::env::var_os("CAMINITO_SOUND_WHEEL_PROBE")
            .expect("CAMINITO_SOUND_WHEEL_PROBE names the built sd-wheel-probe");
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
            Err(err) => panic!("can't accept sd-wheel-probe's connection: {err}"),
        });
        let probe = Probe {
            dir,
            child,
            stdin,
            read: 0,
        };
        probe.wait_for_line(f, "ready");
        probe
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.path()).unwrap_or_default()
    }

    fn path(&self) -> PathBuf {
        self.dir.path().join("log")
    }

    fn wait_for_line(&self, f: &mut Fixture, prefix: &str) {
        let start = self.read;
        let path = self.path();
        f.dispatch_until_explained(
            |_| {
                let log = std::fs::read_to_string(&path).unwrap_or_default();
                log.get(start..)
                    .is_some_and(|log| log.lines().any(|line| line.starts_with(prefix)))
            },
            || std::fs::read_to_string(&path).unwrap_or_default(),
        );
    }

    /// The output's slider's middle in the probe's window, from its ready line.
    fn slider(&self) -> (f64, f64) {
        let log = self.log();
        let line = log.lines().find(|line| line.starts_with("ready ")).unwrap();
        let (x, y) = line["ready slider=".len()..].split_once(',').unwrap();
        (x.parse().unwrap(), y.parse().unwrap())
    }

    fn send(&mut self, line: &str) {
        writeln!(self.stdin, "{line}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// Back to the slider at 50 and the popup at its top.
    fn reset(&mut self, f: &mut Fixture) {
        self.send("reset");
        self.wait_for_line(f, "reset");
        self.read = self.log().len();
    }

    /// Lets the probe handle what it was sent, then returns the slider's and the scroll's
    /// values, and the volumes the popup asked for since the reset.
    fn report(&mut self, f: &mut Fixture, name: &str) -> (f64, f64, Vec<u32>) {
        // A plain wheel must leave the slider alone, which only a time can show: 200 ms is
        // a dozen of the probe's frames, and its answer below comes after GDK has read
        // everything Caminito sent before it.
        f.dispatch_for(Duration::from_millis(200));
        self.send(&format!("state {name}"));
        self.wait_for_line(f, &format!("state {name} "));
        let log = self.log();
        let seen = log[self.read..].to_owned();
        self.read = log.len();
        let state = seen
            .lines()
            .find(|line| line.starts_with(&format!("state {name} ")))
            .unwrap();
        let value = |key: &str| -> f64 {
            let field = state.split(' ').find(|w| w.starts_with(key)).unwrap();
            field[key.len()..].parse().unwrap()
        };
        let asked = seen
            .lines()
            .filter_map(|line| line.strip_prefix("volume="))
            .map(|percent| percent.parse().unwrap())
            .collect();
        (value("scale="), value("vadj="), asked)
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The probe's window's top-left corner, globally, from the dump.
fn probe_origin(f: &Fixture) -> Option<(f64, f64)> {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let windows = dump["windows"].as_array()?;
    let window = windows.iter().find(|w| w["title"] == "sd-wheel-probe")?;
    let g = &window["geometry"];
    Some((g["x"].as_f64()?, g["y"].as_f64()?))
}

#[test]
#[ignore = "needs CAMINITO_SOUND_WHEEL_PROBE, the built sd-wheel-probe"]
fn the_wheel_scrolls_the_sound_popup_and_shift_moves_its_slider() {
    let mut f = Fixture::new();
    f.add_output((1920, 1080), 1.0);
    let mut probe = Probe::start(&mut f);
    f.dispatch_until(|f| probe_origin(f).is_some());
    let (x, y) = probe_origin(&f).unwrap();
    let (slider_x, slider_y) = probe.slider();
    f.pointer_move(x + slider_x, y + slider_y);
    // The pointer's enter, before any wheel: the probe has no state to wait on for it, and
    // 100 ms is several of its frames.
    f.dispatch_for(Duration::from_millis(100));

    let mut results = Vec::new();
    let mut run = |f: &mut Fixture,
                   probe: &mut Probe,
                   name: &str,
                   shift: bool,
                   send: &dyn Fn(&mut Fixture)| {
        probe.reset(f);
        if shift {
            f.key(KEY_LEFTSHIFT, true);
        }
        f.wait_between_inputs(100);
        send(f);
        if shift {
            f.wait_between_inputs(50);
            f.key(KEY_LEFTSHIFT, false);
        }
        // Past the scroll's end, and long enough that a slider moved the wrong way, or a
        // plain wheel taken as a step, would have shown in the frames since.
        f.wait_between_inputs(500);
        results.push((name.to_owned(), probe.report(f, name)));
    };

    let eighths = |f: &mut Fixture| {
        for _ in 0..8 {
            f.scroll(15.);
            // With input_time's own 10 ms, a 16 ms frame apart.
            f.wait_between_inputs(6);
        }
    };

    run(&mut f, &mut probe, "plain-click", false, &|f| {
        f.scroll(CLICK)
    });
    run(&mut f, &mut probe, "shift-click", true, &|f| {
        f.scroll(CLICK)
    });
    run(&mut f, &mut probe, "shift-eighths", true, &eighths);
    run(&mut f, &mut probe, "plain-eighths", false, &eighths);
    run(&mut f, &mut probe, "shift-two-clicks", true, &|f| {
        f.scroll(CLICK);
        f.scroll(CLICK);
    });
    run(&mut f, &mut probe, "shift-fingers", true, &|f| {
        for _ in 0..6 {
            f.finger_scroll(2.5);
            f.wait_between_inputs(6);
        }
        f.finger_scroll(0.);
    });
    // The other way, up, for the sign.
    run(&mut f, &mut probe, "shift-click-up", true, &|f| {
        f.scroll(-CLICK)
    });
    probe.send("quit");

    let result = |name: &str| results.iter().find(|(n, _)| n == name).unwrap().1.clone();
    let close = |a: f64, b: f64| (a - b).abs() < 1e-6;
    // Plain: the slider stays and the popup scrolls, as far for eighths as for a click.
    let (scale, plain_click, asked) = result("plain-click");
    assert!(
        close(scale, 50.) && plain_click > 0. && asked.is_empty(),
        "{scale} {plain_click}"
    );
    let (scale, plain_eighths, asked) = result("plain-eighths");
    assert!(close(scale, 50.) && close(plain_eighths, plain_click) && asked.is_empty());
    // Shift: volume-step a click, whole or in eighths, and the popup stays.
    for (name, expected) in [
        ("shift-click", 45.),
        ("shift-eighths", 45.),
        ("shift-two-clicks", 40.),
        ("shift-click-up", 55.),
        // A touchpad has no clicks to count: Shift leaves the volume alone.
        ("shift-fingers", 50.),
    ] {
        let (scale, vadj, asked) = result(name);
        assert!(
            close(scale, expected) && close(vadj, 0.),
            "{name}: {scale}, {vadj}"
        );
        // What the server is asked for last is the slider's, rounded.
        if expected != 50. {
            assert_eq!(asked.last().copied(), Some(expected as u32), "{name}");
        }
    }
}
