//! The Dock applet's items and clicks end to end (CMN-219), and its launch and attention
//! feedback (CMN-221): `dk-probe`, the dock in a toplevel
//! of its own, runs on GDK's Wayland backend against Caminito, reads the windows through
//! libxfce4windowing as mate-panel's window list does (ADR 0013), and is clicked with the
//! pointer the way a user would. Desktop entries in a directory of the test's own say what a
//! launch runs: each appends a line to a file named for it.
//!
//! The Trash (CMN-220) is at the end of the row. It reads `trash:///` through gvfs, which
//! the tests that need it give a session bus of its own, with `XDG_DATA_HOME` the test's, so
//! that the Trash is the test's too; they need gvfs installed, as Caja does.
//!
//! The Kept folders (CMN-239) are between the separator and the Trash. They need no gvfs: the
//! folders are the test's own, and a move or a copy goes through GIO's local files.
//!
//! The app id to item resolution has its own tests without a panel, in
//! `helpers/caminito-dock-applet/test-resolve.c` and `test-model.c`; what this adds is what
//! the compositor does with the clicks: activating a window on another workspace, minimising
//! into the item's rectangle, and the animation that follows.
//!
//! It needs the built probe, named by `CAMINITO_DOCK_PROBE`, so it's ignored by default and
//! `tools/check` runs it after the C helpers' build. With `CAMINITO_DOCK_SHOTS` naming a
//! directory, the scenarios also leave a picture of the output there, as a PPM.

use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::time::Duration;

use caminito_policy::FocusNewWindows;
use smithay::utils::{Logical, Point, Rectangle};

use super::fixture::{
    BTN_LEFT, BTN_MIDDLE, BTN_RIGHT, ClientId, ClientWindow, Fixture, Kind, WINDOW_COLOR,
    WindowSpec,
};
use crate::minimize_animation::LENGTH;
use crate::settings::Value;
use crate::settings::translate::MARCO_GENERAL;
use crate::state::SEAT;

/// The output the dock and the windows are on.
const OUTPUT: (i32, i32) = (1280, 720);

/// How far into the minimise animation the tests read it.
const PART_WAY: Duration = Duration::from_millis(100);

/// The desktop entries of the test's own: id, name, `StartupWMClass`, and an action.
const ENTRIES: &[(&str, &str, Option<&str>, Option<&str>)] = &[
    ("mate-terminal", "Terminal", None, Some("New Window")),
    (
        "firefox",
        "Firefox",
        Some("Navigator"),
        Some("New Private Window"),
    ),
    ("caja", "Files", None, None),
];

/// One item of the probe's answer to `items`.
#[derive(Debug, Clone)]
struct Item {
    key: String,
    windows: usize,
    active: bool,
    kept: bool,
    /// The icon pulses: the dock launched the program and no window of it has mapped (CMN-221).
    pulsing: bool,
    /// The icon bounces, and `highlighted` is kept after the last bounce.
    bouncing: bool,
    /// A window of the item demands attention and hasn't had the focus since.
    highlighted: bool,
    /// The item's rectangle in the probe's window.
    rect: Rectangle<i32, Logical>,
    /// The title of the item's most recent window.
    title: String,
}

/// The probe in a scratch directory with its log, its settings and its desktop entries.
struct Probe {
    dir: tempfile::TempDir,
    child: Child,
    stdin: ChildStdin,
    /// How much of the log the scenarios so far have read.
    read: usize,
    /// The edge of the screen it's along, and how thick it is.
    edge: &'static str,
    thickness: i32,
    /// The session bus gvfs's Trash is reached through, for the tests that have one.
    bus: Option<Child>,
}

/// The Trash as the dock shows it.
#[derive(Debug, Clone)]
struct Trash {
    full: bool,
    /// The Trash's rectangle in the probe's window.
    rect: Rectangle<i32, Logical>,
}

/// A Kept folder as the dock shows it.
#[derive(Debug, Clone)]
struct Folder {
    uri: String,
    gone: bool,
    /// The folder's rectangle in the probe's window.
    rect: Rectangle<i32, Logical>,
}

impl Probe {
    fn start(
        f: &mut Fixture,
        dir: tempfile::TempDir,
        edge: &'static str,
        size: i32,
        gvfs: bool,
    ) -> Self {
        let program = PathBuf::from(
            std::env::var_os("CAMINITO_DOCK_PROBE").expect("CAMINITO_DOCK_PROBE names the probe"),
        );
        let applications = dir.path().join("data/applications");
        std::fs::create_dir_all(&applications).unwrap();
        // What a launch runs: a line in the entry's file, with the activation token the
        // launch carried, as a program would read it from its environment (CMN-221).
        let script = dir.path().join("log-launch");
        std::fs::write(
            &script,
            "#!/bin/sh\necho \"$1 xdg=${XDG_ACTIVATION_TOKEN} dsi=${DESKTOP_STARTUP_ID}\" >> \"$2\"\n",
        )
        .unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        for (id, name, class, action) in ENTRIES {
            let run = |what: &str| {
                format!(
                    "{} {what} {}",
                    script.display(),
                    dir.path().join(format!("launched-{id}")).display()
                )
            };
            let mut entry = format!(
                "[Desktop Entry]\nType=Application\nName={name}\nExec={}\n",
                run("run")
            );
            if let Some(class) = class {
                entry += &format!("StartupWMClass={class}\n");
            }
            if let Some(action) = action {
                entry += &format!(
                    "Actions=extra;\n\n[Desktop Action extra]\nName={action}\nExec={}\n",
                    run("extra")
                );
            }
            std::fs::write(applications.join(format!("{id}.desktop")), entry).unwrap();
        }
        // What opens a folder: Caja's handler on a Session. It records the URIs it is given.
        std::fs::write(
            applications.join("caja-folder-handler.desktop"),
            format!(
                "[Desktop Entry]\nType=Application\nName=Caja\nNoDisplay=true\nExec=sh -c \"echo %U >> {}\"\n",
                dir.path().join("opened-in-caja").display()
            ),
        )
        .unwrap();
        // The system's icons, for the items and the menu's, and what the image loaders need
        // to read them: the types of the images, and their own programs.
        for shared in ["icons", "mime", "glycin-loaders"] {
            let _ = std::os::unix::fs::symlink(
                Path::new("/usr/share").join(shared),
                dir.path().join("data").join(shared),
            );
        }
        let listener = UnixListener::bind(dir.path().join("wayland-test")).unwrap();
        listener.set_nonblocking(true).unwrap();
        let log = std::fs::File::options()
            .create(true)
            .append(true)
            .open(dir.path().join("log"))
            .unwrap();
        // The Trash: gvfs's trash daemon, which a bus starts, over this test's data directory.
        // The bus itself looks for the daemon's service file where the system keeps them.
        let (bus, bus_address) = if gvfs {
            let mut child = Command::new("dbus-daemon")
                .args(["--session", "--nofork", "--print-address=1"])
                .arg(format!(
                    "--address=unix:path={}",
                    dir.path().join("bus").display()
                ))
                .env("HOME", dir.path())
                .env("XDG_DATA_HOME", dir.path().join("data"))
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .expect("dbus-daemon starts");
            // The bus prints its address once it listens.
            let mut address = String::new();
            BufReader::new(child.stdout.take().unwrap())
                .read_line(&mut address)
                .unwrap();
            assert!(!address.trim().is_empty(), "the session bus listens");
            (Some(child), address.trim().to_owned())
        } else {
            (None, "unix:path=/nonexistent".to_owned())
        };
        let mut command = Command::new(&program);
        command
            .arg(format!("--edge={edge}"))
            .arg(format!("--size={size}"))
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", dir.path())
            .env("XDG_RUNTIME_DIR", dir.path())
            .env("XDG_CONFIG_HOME", dir.path().join("config"))
            .env("XDG_DATA_HOME", dir.path().join("data"))
            .env("XDG_DATA_DIRS", dir.path().join("data"))
            .env("WAYLAND_DISPLAY", "wayland-test")
            .env("GDK_BACKEND", "wayland")
            .env("GTK_THEME", "Adwaita")
            .env("NO_AT_BRIDGE", "1")
            // The settings in a keyfile of the test's, which a second probe reads back.
            .env("GSETTINGS_BACKEND", "keyfile")
            .env("GSETTINGS_SCHEMA_DIR", program.parent().unwrap())
            .env("DBUS_SESSION_BUS_ADDRESS", bus_address)
            .stdin(Stdio::piped())
            .stdout(log.try_clone().unwrap())
            .stderr(log);
        // Where libxfce4windowing isn't in the system's directories, and to look into the probe.
        for name in [
            "LD_LIBRARY_PATH",
            "G_MESSAGES_DEBUG",
            "WAYLAND_DEBUG",
            "GIO_EXTRA_MODULES",
        ] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        let mut child = command.spawn().unwrap();
        let pid = child.id();
        let stdin = child.stdin.take().unwrap();
        f.dispatch_until_mut(|f| match listener.accept() {
            Ok((stream, _)) => {
                f.insert_stream(stream, Some(pid));
                true
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => false,
            Err(err) => panic!("can't accept dk-probe's connection: {err}"),
        });
        let probe = Probe {
            dir,
            child,
            stdin,
            read: 0,
            edge,
            thickness: size,
            bus,
        };
        probe.wait_for_line(f, "ready");
        probe
    }

    fn log(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("log")).unwrap_or_default()
    }

    fn wait_for_line(&self, f: &mut Fixture, prefix: &str) {
        let start = self.read;
        let path = self.dir.path().join("log");
        f.dispatch_until_explained(
            |_| {
                let log = std::fs::read_to_string(&path).unwrap_or_default();
                log.get(start..)
                    .is_some_and(|log| log.lines().any(|line| line.starts_with(prefix)))
            },
            || {
                let log = std::fs::read_to_string(&path).unwrap_or_default();
                format!("no {prefix:?} line in the probe's log: {log}")
            },
        );
    }

    fn send(&mut self, line: &str) {
        writeln!(self.stdin, "{line}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// Sends `command` and returns the lines of the answer that end with `end`, without it.
    fn ask(&mut self, f: &mut Fixture, command: &str, end: &str) -> Vec<String> {
        self.read = self.log().len();
        self.send(command);
        self.wait_for_line(f, end);
        let log = self.log();
        let seen = &log[self.read..];
        let mut lines = Vec::new();
        for line in seen.lines() {
            if line.starts_with(end) {
                break;
            }
            lines.push(line.to_owned());
        }
        self.read = log.len();
        lines
    }

    /// The items in the dock now.
    fn items(&mut self, f: &mut Fixture) -> Vec<Item> {
        self.ask(f, "items", "items-end")
            .iter()
            .filter_map(|line| line.strip_prefix("item "))
            .map(|line| {
                let (line, title) = line.split_once(" title=").unwrap();
                let mut words = line.split(' ');
                let key = words.next().unwrap().to_owned();
                let field =
                    |word: &str| -> i32 { word.split_once('=').unwrap().1.parse().unwrap() };
                let mut fields = words.map(field);
                let mut next = || fields.next().unwrap();
                let (windows, active, kept) = (next(), next(), next());
                let (pulsing, bouncing, highlighted) = (next(), next(), next());
                let (x, y, w, h) = (next(), next(), next(), next());
                Item {
                    key,
                    windows: windows as usize,
                    active: active != 0,
                    kept: kept != 0,
                    pulsing: pulsing != 0,
                    bouncing: bouncing != 0,
                    highlighted: highlighted != 0,
                    rect: Rectangle::new((x, y).into(), (w, h).into()),
                    title: title.to_owned(),
                }
            })
            .collect()
    }

    /// What the dock's lookup of Caminito's name makes of each window libxfce4windowing
    /// lists (CMN-243): the title, the identifier or none, and whether the ext handle with
    /// that identifier was found.
    fn identifiers(&mut self, f: &mut Fixture) -> Vec<(String, Option<String>, bool)> {
        self.ask(f, "identifiers", "identifiers-end")
            .iter()
            .filter_map(|line| line.strip_prefix("toplevel "))
            .map(|line| {
                let (line, title) = line.split_once(" title=").unwrap();
                let mut words = line.split(' ');
                let identifier = words.next().unwrap();
                let found = words.next().unwrap() == "1";
                let identifier = (identifier != "-").then(|| identifier.to_owned());
                (title.to_owned(), identifier, found)
            })
            .collect()
    }

    /// Dispatches until the dock's items satisfy `done`, and returns them.
    #[track_caller]
    fn wait_for_items(&mut self, f: &mut Fixture, done: impl Fn(&[Item]) -> bool) -> Vec<Item> {
        let mut found = None;
        f.dispatch_until_mut(|f| {
            let items = self.items(f);
            // The window takes a round trip to the compositor to grow to its items.
            let holds = items.iter().all(|item| item.rect.size.w > 1) && done(&items);
            if holds {
                found = Some(items);
            }
            holds
        });
        found.unwrap()
    }

    fn menu(&mut self, f: &mut Fixture) -> Vec<String> {
        self.ask(f, "menu", "menu-end")
            .iter()
            .filter_map(|line| line.strip_prefix("menu ").map(str::to_owned))
            .collect()
    }

    /// The labels of the open menu's items that can't be chosen.
    fn menu_disabled(&mut self, f: &mut Fixture) -> Vec<String> {
        self.ask(f, "menu", "menu-end")
            .iter()
            .filter_map(|line| line.strip_prefix("menu-disabled ").map(str::to_owned))
            .collect()
    }

    /// The Trash as the dock shows it now.
    fn trash(&mut self, f: &mut Fixture) -> Trash {
        let lines = self.ask(f, "trash", "trash-end");
        let line = lines
            .iter()
            .find_map(|line| line.strip_prefix("trash "))
            .expect("the probe answers with the Trash");
        let fields: Vec<i32> = line
            .split(' ')
            .map(|word| word.split_once('=').unwrap().1.parse().unwrap())
            .collect();
        Trash {
            full: fields[0] != 0,
            rect: Rectangle::new((fields[1], fields[2]).into(), (fields[3], fields[4]).into()),
        }
    }

    /// The Kept folders in the dock now, in order.
    fn folders(&mut self, f: &mut Fixture) -> Vec<Folder> {
        self.ask(f, "folders", "folders-end")
            .iter()
            .filter_map(|line| line.strip_prefix("folder "))
            .map(|line| {
                let mut words = line.split(' ');
                let uri = words.next().unwrap().to_owned();
                let fields: Vec<i32> = words
                    .map(|word| word.split_once('=').unwrap().1.parse().unwrap())
                    .collect();
                Folder {
                    uri,
                    gone: fields[0] != 0,
                    rect: Rectangle::new(
                        (fields[1], fields[2]).into(),
                        (fields[3], fields[4]).into(),
                    ),
                }
            })
            .collect()
    }

    /// The cells of the grid popup being shown, a folder's with a slash after.
    fn grid(&mut self, f: &mut Fixture) -> Vec<String> {
        self.ask(f, "grid", "grid-end")
            .iter()
            .filter_map(|line| line.strip_prefix("grid ").map(str::to_owned))
            .collect()
    }

    /// The title of the error being shown, none if there isn't one.
    fn error(&mut self, f: &mut Fixture) -> Vec<String> {
        self.ask(f, "error", "error-end")
            .iter()
            .filter_map(|line| line.strip_prefix("error ").map(str::to_owned))
            .collect()
    }

    /// A folder of the test's own, with a file in it, and its URI.
    fn folder(&self, name: &str) -> (PathBuf, String) {
        let path = self.dir.path().join("folders").join(name);
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("inside.txt"), "content").unwrap();
        let uri = format!("file://{}", path.display());
        (path, uri)
    }

    /// The titles of the confirmation being shown, none if there isn't one.
    fn dialog(&mut self, f: &mut Fixture) -> Vec<String> {
        self.ask(f, "dialog", "dialog-end")
            .iter()
            .filter_map(|line| line.strip_prefix("dialog ").map(str::to_owned))
            .collect()
    }

    /// The names in the Trash's folder, which gvfs's `trash:///` is.
    fn trashed(&self) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(self.dir.path().join("data/Trash/files"))
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// A file of the test's own, outside the Trash, and its URI.
    fn file(&self, name: &str) -> (PathBuf, String) {
        let path = self.dir.path().join("outside").join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "content").unwrap();
        let uri = format!("file://{}", path.display());
        (path, uri)
    }

    /// The URIs Caja was asked to open.
    fn opened_in_caja(&self) -> Vec<String> {
        std::fs::read_to_string(self.dir.path().join("opened-in-caja"))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// How many times the entry's command has run.
    fn launches(&self, id: &str) -> usize {
        std::fs::read_to_string(self.dir.path().join(format!("launched-{id}")))
            .map_or(0, |launched| launched.lines().count())
    }

    /// The lines the entry's command has written, `run xdg=TOKEN dsi=ID` for each launch.
    fn launch_lines(&self, id: &str) -> Vec<String> {
        std::fs::read_to_string(self.dir.path().join(format!("launched-{id}")))
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    /// The probe's surface, along its edge of the screen as a panel is.
    fn surface(&self) -> Rectangle<i32, Logical> {
        let t = self.thickness;
        match self.edge {
            "top" => Rectangle::new((0, 0).into(), (OUTPUT.0, t).into()),
            "left" => Rectangle::new((0, 0).into(), (t, OUTPUT.1).into()),
            "right" => Rectangle::new((OUTPUT.0 - t, 0).into(), (t, OUTPUT.1).into()),
            _ => Rectangle::new((0, OUTPUT.1 - t).into(), (OUTPUT.0, t).into()),
        }
    }

    /// Where an item is, globally.
    fn rect_of(&self, item: &Item) -> Rectangle<i32, Logical> {
        self.global(item.rect)
    }

    fn global(&self, rect: Rectangle<i32, Logical>) -> Rectangle<i32, Logical> {
        Rectangle::new(rect.loc + self.surface().loc, rect.size)
    }

    fn middle_of_rect(&self, rect: Rectangle<i32, Logical>) -> (f64, f64) {
        let rect = self.global(rect);
        (
            f64::from(rect.loc.x) + f64::from(rect.size.w) / 2.,
            f64::from(rect.loc.y) + f64::from(rect.size.h) / 2.,
        )
    }

    /// Ends the probe the way the panel's end does, which takes the compositor's answers to
    /// let go of its surfaces, and returns the directory with its settings for the next.
    fn stop(mut self, f: &mut Fixture) -> tempfile::TempDir {
        self.send("quit");
        f.dispatch_until_mut(|_| self.child.try_wait().unwrap().is_some());
        let dir = self.dir.path().to_owned();
        let moved = tempfile::TempDir::new_in(dir.parent().unwrap()).unwrap();
        for entry in ["config", "data"] {
            std::fs::rename(dir.join(entry), moved.path().join(entry)).unwrap();
        }
        moved
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(bus) = &mut self.bus {
            let _ = bus.kill();
            let _ = bus.wait();
        }
    }
}

/// The output, a dock along the bottom edge of the screen, and the windows of the programs.
struct Desk {
    f: Fixture,
    probe: Probe,
}

impl Desk {
    fn new() -> Self {
        Self::with("bottom", 48)
    }

    /// A dock whose Trash is gvfs's, on a session bus of its own.
    fn with_gvfs() -> Self {
        Self::start("bottom", 48, true)
    }

    /// A dock along `edge` of the screen, whose items are `size` pixels square.
    fn with(edge: &'static str, size: i32) -> Self {
        Self::start(edge, size, false)
    }

    fn start(edge: &'static str, size: i32, gvfs: bool) -> Self {
        let mut f = Fixture::new();
        f.add_output(OUTPUT, 1.0);
        let mut probe = Probe::start(&mut f, tempfile::tempdir().unwrap(), edge, size, gvfs);
        probe.wait_for_items(&mut f, |_| true);
        Desk { f, probe }
    }

    /// Maps a window of a client of its own, small enough to leave the dock clear.
    fn open(&mut self, app_id: &'static str, title: &'static str) -> ClientWindow {
        let client = self.f.connect(Kind::Xdg);
        self.open_as(client, app_id, title)
    }

    fn open_as(
        &mut self,
        client: ClientId,
        app_id: &'static str,
        title: &'static str,
    ) -> ClientWindow {
        let window = self.f.map(
            client,
            WindowSpec {
                title: Some(title),
                app_id: Some(app_id),
                ..WindowSpec::plain((240, 160))
            },
        );
        // The dock has seen it take the focus before the next window comes: windows that come
        // within a millisecond of each other leave it no order to go by.
        self.wait_for_focus(window);
        self.probe.wait_for_items(&mut self.f, |items| {
            items.iter().any(|item| item.active && item.title == title)
        });
        let dock = self.probe.surface();
        let geometry = self.f.geometry(window);
        assert!(
            !geometry.overlaps(dock),
            "{app_id}'s window at {geometry:?} covers the dock at {dock:?}"
        );
        window
    }

    /// The items once the dock has laid them out.
    fn items(&mut self) -> Vec<Item> {
        self.probe.wait_for_items(&mut self.f, |_| true)
    }

    #[track_caller]
    fn item(&mut self, key: &str) -> Item {
        self.items()
            .into_iter()
            .find(|item| item.key == key)
            .unwrap_or_else(|| panic!("an item for {key}"))
    }

    /// Waits for the item with `key` to have `windows` windows, and returns it.
    #[track_caller]
    fn wait_for(&mut self, key: &str, windows: usize) -> Item {
        let items = self.probe.wait_for_items(&mut self.f, |items| {
            items
                .iter()
                .any(|item| item.key == key && item.windows == windows)
        });
        items.into_iter().find(|item| item.key == key).unwrap()
    }

    /// Dispatches until the entry's command has run `times` times.
    #[track_caller]
    fn wait_for_launches(&mut self, id: &str, times: usize) {
        let probe = &self.probe;
        self.f.dispatch_until_explained(
            |_| probe.launches(id) == times,
            || {
                format!(
                    "{id} was launched {} times: {}",
                    probe.launches(id),
                    probe.log()
                )
            },
        );
    }

    fn press(&mut self, item: &Item, button: u32) {
        self.press_rect(item.rect, button);
    }

    fn press_rect(&mut self, rect: Rectangle<i32, Logical>, button: u32) {
        let (x, y) = self.probe.middle_of_rect(rect);
        self.f.pointer_move(x, y);
        self.f.pointer_button(button, true);
        self.f.pointer_button(button, false);
    }

    /// The Trash once the dock has laid it out.
    fn trash(&mut self) -> Trash {
        let mut found = None;
        self.f.dispatch_until_mut(|f| {
            let trash = self.probe.trash(f);
            let laid_out = trash.rect.size.w > 1;
            if laid_out {
                found = Some(trash);
            }
            laid_out
        });
        found.unwrap()
    }

    /// Dispatches until the Kept folders satisfy `done` once laid out, and returns them.
    #[track_caller]
    fn wait_for_folders(&mut self, done: impl Fn(&[Folder]) -> bool) -> Vec<Folder> {
        let mut found = None;
        self.f.dispatch_until_mut(|f| {
            let folders = self.probe.folders(f);
            let holds = folders.iter().all(|folder| folder.rect.size.w > 1) && done(&folders);
            if holds {
                found = Some(folders);
            }
            holds
        });
        found.unwrap()
    }

    /// Dispatches until the dock shows the Trash full or empty, and returns it.
    #[track_caller]
    fn wait_for_trash(&mut self, full: bool) -> Trash {
        let mut found = None;
        self.f.dispatch_until_mut(|f| {
            let trash = self.probe.trash(f);
            let holds = trash.full == full && trash.rect.size.w > 1;
            if holds {
                found = Some(trash);
            }
            holds
        });
        found.unwrap()
    }

    /// Dispatches until the menu has labels, which it takes the probe a turn to show.
    fn wait_for_menu(&mut self) -> Vec<String> {
        let mut labels = Vec::new();
        self.f.dispatch_until_mut(|f| {
            labels = self.probe.menu(f);
            !labels.is_empty()
        });
        labels
    }

    fn click(&mut self, item: &Item) {
        self.press(item, BTN_LEFT);
    }

    fn focused(&self) -> Option<caminito_policy::WindowId> {
        self.f.state.caminito.policy.focus(SEAT)
    }

    #[track_caller]
    fn wait_for_focus(&mut self, window: ClientWindow) {
        let pid = self.f.pid(window);
        let probe = &self.probe;
        self.f.dispatch_until_explained(
            |f| f.state.caminito.policy.focus(SEAT) == Some(pid),
            || format!("{pid:?} never had the focus: {}", probe.log()),
        );
    }

    /// Maps a window of a client of the process `pid`, which policy tells a client's windows by.
    fn open_process(
        &mut self,
        pid: u32,
        app_id: &'static str,
        title: &'static str,
    ) -> ClientWindow {
        let client = self.f.connect_as_process(pid);
        self.open_as(client, app_id, title)
    }

    /// Maps a window of a client that has the pointer and keyboard, as a toolkit has them, so
    /// that it can ask for an activation token with the input it has had.
    fn open_with_input(&mut self, app_id: &'static str, title: &'static str) -> ClientWindow {
        let client = self.f.connect(Kind::Xdg);
        self.f.client(client).bind_input();
        self.f.roundtrip(client);
        self.open_as(client, app_id, title)
    }

    /// From now on `focus-new-windows` is `mode`: under strict a window that isn't focused is
    /// refused the focus by an activation, and demands attention; under smart a window of
    /// another client maps with the focus only with a recent token (165, 166).
    fn focus_new_windows(&mut self, mode: &str, expected: FocusNewWindows) {
        self.f
            .set_setting(MARCO_GENERAL, "focus-new-windows", Value::Str(mode.into()));
        self.f
            .dispatch_until(|f| f.state.caminito.settings.policy.focus_new_windows == expected);
    }

    fn strict(&mut self) {
        self.focus_new_windows("strict", FocusNewWindows::Strict);
    }

    /// Maps a window of `id`, as a launched program does: with the activation token, if it has
    /// one, used before the window has content.
    fn map_launched(
        &mut self,
        id: ClientId,
        app_id: &'static str,
        title: &'static str,
        token: Option<&str>,
    ) -> ClientWindow {
        let window = self.f.client(id).create_window();
        self.f.client(id).set_title(window, title);
        self.f.client(id).set_app_id(window, app_id);
        self.f.client(id).commit(window);
        self.f.roundtrip(id);
        if let Some(token) = token {
            self.f.client(id).activate(token, window);
        }
        self.f
            .client(id)
            .ack_and_commit_buffer(window, (240, 160), WINDOW_COLOR);
        self.f.roundtrip(id);
        ClientWindow::Xdg(id, window)
    }

    /// A token from `from`'s client with the input serial it has had.
    fn token_from(&mut self, from: ClientWindow) -> String {
        let id = from.client();
        let serial = self.f.client_state(id).last_input_serial;
        assert!(serial.is_some(), "{from:?}'s client has had input");
        let token = self
            .f
            .client(id)
            .request_activation_token(serial, Some(from.xdg()));
        self.f.roundtrip(id);
        self.f
            .client(id)
            .activation_token(token)
            .unwrap()
            .to_owned()
    }

    /// The activation token the launch of the entry's command carried.
    #[track_caller]
    fn launch_token(&self, id: &str) -> String {
        let lines = self.probe.launch_lines(id);
        let line = lines.last().unwrap_or_else(|| panic!("{id} was launched"));
        line.split_whitespace()
            .find_map(|word| word.strip_prefix("xdg="))
            .unwrap_or("")
            .to_owned()
    }

    /// Dispatches until the item with `key` satisfies `done`, and returns it.
    #[track_caller]
    fn wait_for_item(&mut self, key: &str, done: impl Fn(&Item) -> bool) -> Item {
        let items = self.probe.wait_for_items(&mut self.f, |items| {
            items.iter().any(|item| item.key == key && done(item))
        });
        items.into_iter().find(|item| item.key == key).unwrap()
    }

    /// `count` pictures of the output, `name` and a number, one after another: the frames of
    /// an animation.
    fn frames(&mut self, name: &str, count: usize) {
        for i in 1..=count {
            self.shot(&format!("{name}-{i}"));
        }
    }

    /// Leaves a picture of the output where `CAMINITO_DOCK_SHOTS` says, as a PPM.
    fn shot(&mut self, name: &str) {
        let Some(dir) = std::env::var_os("CAMINITO_DOCK_SHOTS") else {
            return;
        };
        // Let the probe draw what it was last told.
        self.f.dispatch_for(Duration::from_millis(150));
        let output = self.f.state.caminito.outputs[0].clone();
        let pixels = self
            .f
            .state
            .backend
            .headless_mut()
            .pixels(&output)
            .expect("the output is drawn");
        let mut ppm = format!("P6\n{} {}\n255\n", OUTPUT.0, OUTPUT.1).into_bytes();
        for pixel in pixels {
            ppm.extend([(pixel >> 16) as u8, (pixel >> 8) as u8, pixel as u8]);
        }
        std::fs::write(Path::new(&dir).join(format!("{name}.ppm")), ppm).unwrap();
    }
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_click_launches_a_kept_program_that_is_not_running() {
    let mut d = Desk::new();
    d.probe.send("keep mate-terminal.desktop");
    let item = d.wait_for("mate-terminal.desktop", 0);
    assert!(item.kept);
    assert_eq!(d.probe.launches("mate-terminal"), 0);

    d.click(&item);

    d.wait_for_launches("mate-terminal", 1);
    d.shot("01-kept-items");
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_click_activates_the_window_of_a_running_program() {
    let mut d = Desk::new();
    let terminal = d.open("mate-terminal", "Terminal");
    let _other = d.open("caja", "Files");
    let item = d.wait_for("mate-terminal.desktop", 1);
    assert!(!item.active, "the Files window has the focus");
    assert!(!item.kept);

    d.click(&item);

    d.wait_for_focus(terminal);
    assert_eq!(d.probe.launches("mate-terminal"), 0, "nothing is launched");
    let item = d.item("mate-terminal.desktop");
    assert!(item.active);
    assert!(!d.item("caja.desktop").active);
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_click_on_the_focused_programs_one_window_minimises_it_and_the_next_restores_it() {
    let mut d = Desk::new();
    let terminal = d.open("mate-terminal", "Terminal");
    let item = d.wait_for("mate-terminal.desktop", 1);
    d.wait_for_focus(terminal);
    let item = Item {
        active: true,
        ..item
    };
    assert!(d.item("mate-terminal.desktop").active);

    d.click(&item);
    let pid = d.f.pid(terminal);
    d.f.dispatch_until(|f| f.state.caminito.policy.is_minimized(pid));

    d.click(&item);
    d.f.dispatch_until(|f| !f.state.caminito.policy.is_minimized(pid));
    d.wait_for_focus(terminal);
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_click_on_a_program_with_several_windows_activates_the_most_recent_one() {
    let mut d = Desk::new();
    let first = d.open("mate-terminal", "First");
    let second = d.open("mate-terminal", "Second");
    let other = d.open("caja", "Files");
    let item = d.wait_for("mate-terminal.desktop", 2);
    assert_eq!(d.items().len(), 2, "two processes, one item");
    d.shot("06-two-processes-one-item");

    // The first is the one the user used last. The dock sees each focus before the next, as it
    // does when a person's clicks are that far apart.
    let now = d.f.state.caminito.now();
    d.f.state
        .caminito
        .policy
        .activate(SEAT, d.f.pid(first), now);
    d.wait_for_focus(first);
    d.probe.wait_for_items(&mut d.f, |items| {
        items
            .iter()
            .any(|item| item.key == "mate-terminal.desktop" && item.active)
    });
    let now = d.f.state.caminito.now();
    d.f.state
        .caminito
        .policy
        .activate(SEAT, d.f.pid(other), now);
    d.wait_for_focus(other);
    d.probe.wait_for_items(&mut d.f, |items| {
        items
            .iter()
            .any(|item| item.key == "caja.desktop" && item.active)
    });

    d.click(&item);

    d.wait_for_focus(first);
    assert_ne!(d.focused(), Some(d.f.pid(second)));
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_click_on_a_window_on_another_workspace_switches_to_it() {
    let mut d = Desk::new();
    let terminal = d.open("mate-terminal", "Terminal");
    let item = d.wait_for("mate-terminal.desktop", 1);
    let pid = d.f.pid(terminal);
    d.f.state.caminito.policy.move_to_workspace(pid, 1);
    d.f.dispatch();
    assert_eq!(d.f.state.caminito.policy.active_workspace(), 0);

    d.click(&item);

    d.f.dispatch_until(|f| f.state.caminito.policy.active_workspace() == 1);
    d.wait_for_focus(terminal);
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn middle_click_launches_a_new_instance_of_a_running_program() {
    let mut d = Desk::new();
    let _terminal = d.open("mate-terminal", "Terminal");
    let item = d.wait_for("mate-terminal.desktop", 1);

    d.press(&item, BTN_MIDDLE);

    d.wait_for_launches("mate-terminal", 1);
    d.press(&item, BTN_MIDDLE);
    d.wait_for_launches("mate-terminal", 2);
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_window_with_no_desktop_entry_has_an_item_of_its_own() {
    let mut d = Desk::new();
    let _mystery = d.open("mystery-app", "Mystery");
    let again = d.open("mystery-app", "Mystery again");
    let terminal = d.open("mate-terminal", "Terminal");
    let items = d.probe.wait_for_items(&mut d.f, |items| items.len() == 2);

    let item = items
        .iter()
        .find(|i| i.key == "window:mystery-app")
        .unwrap();
    assert_eq!(item.windows, 2);
    assert!(!item.kept);
    assert_eq!(
        items[1].key, "mate-terminal.desktop",
        "in the order they opened"
    );
    d.shot("02-running-items");

    // It is clicked as any other item is: the terminal has the focus, and the most recent of
    // the item's windows is the one the click brings forward.
    d.wait_for_focus(terminal);
    d.probe.wait_for_items(&mut d.f, |items| {
        items
            .iter()
            .any(|item| item.key == "mate-terminal.desktop" && item.active)
    });
    d.click(item);
    d.wait_for_focus(again);
    d.shot("03-focused-indicator");
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn an_xwayland_window_joins_the_item_its_startup_wm_class_names() {
    let mut d = Desk::new();
    d.f.start_xwayland();
    let x = d.f.connect(Kind::X11);
    let _window = d.open_as(x, "Navigator", "Firefox");

    let item = d.wait_for("firefox.desktop", 1);

    assert!(!item.kept);
    assert_eq!(d.items().len(), 1, "and no item of its own");
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_minimised_window_animates_into_its_dock_item() {
    let mut d = Desk::new();
    // A kept item before the terminal's, so that its item isn't at the dock's origin.
    d.probe.send("keep caja.desktop");
    d.wait_for("caja.desktop", 0);
    let terminal = d.open("mate-terminal", "Terminal");
    let item = d.wait_for("mate-terminal.desktop", 1);
    assert_ne!(item.rect.loc, Point::from((0, 0)), "{item:?}");
    d.wait_for_focus(terminal);
    let target = d.probe.rect_of(&item);
    let pid = d.f.pid(terminal);
    // The window's rectangle is the item's in the dock's surface, set when the window came.
    d.f.dispatch_until(|f| {
        f.state
            .caminito
            .policy
            .window(pid)
            .and_then(|mapped| mapped.minimize_rectangle())
            .is_some_and(|(_, rect)| rect == item.rect)
    });

    let frame = d.f.visible_frame(terminal);
    let item = Item {
        active: true,
        ..item
    };
    // The probe takes real time to answer the click: only the test moves the clock on.
    d.f.stop_clock();
    d.click(&item);
    d.f.dispatch_until(|f| f.state.caminito.policy.is_minimized(pid));
    d.f.advance_clock(PART_WAY);

    // A spot the window covers 100 ms into the 250 of its way to the item, which neither it
    // nor the item does at rest. The clock is stopped, so the way is exactly two fifths.
    let at = |from: i32, to: i32| from + (to - from) * 2 / 5;
    let way = Rectangle::new(
        (at(frame.loc.x, target.loc.x), at(frame.loc.y, target.loc.y)).into(),
        (
            at(frame.size.w, target.size.w),
            at(frame.size.h, target.size.h),
        )
            .into(),
    );
    let spot = way.loc + Point::from((way.size.w / 2, way.size.h / 2));
    assert!(!frame.contains(spot) && !target.contains(spot), "{spot:?}");
    let output = d.f.state.caminito.outputs[0].clone();
    let pixel =
        d.f.state
            .backend
            .headless_mut()
            .pixel(&output, spot.to_physical(1))
            .expect("the output is drawn")
            & 0xff_ffff;
    assert_eq!(pixel, WINDOW_COLOR & 0xff_ffff, "it's on its way");

    d.f.advance_clock(LENGTH - PART_WAY);
}

/// The ways a menu is asked for are a right-click on the item.
#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn the_context_menu_has_the_windows_the_actions_keep_and_close_all() {
    let mut d = Desk::new();
    let first = d.open("mate-terminal", "First terminal");
    let second = d.open("mate-terminal", "Second terminal");
    let item = d.wait_for("mate-terminal.desktop", 2);

    d.press(&item, BTN_RIGHT);
    let mut labels = Vec::new();
    d.f.dispatch_until_mut(|f| {
        labels = d.probe.menu(f);
        !labels.is_empty()
    });
    assert_eq!(
        labels,
        [
            "Second terminal",
            "First terminal",
            "---",
            "New Window",
            "---",
            "Keep in Dock",
            "Close all windows",
        ]
        .map(str::to_owned),
        "the windows, most recent first, the actions, keep and close"
    );
    d.shot("04-context-menu");

    // An action runs the entry's command for it.
    d.probe.send("choose New Window");
    d.wait_for_launches("mate-terminal", 1);

    // Keep, then the menu says remove.
    d.press(&item, BTN_RIGHT);
    d.f.dispatch_until_mut(|f| !d.probe.menu(f).is_empty());
    d.probe.send("choose Keep in Dock");
    let kept = d.probe.wait_for_items(&mut d.f, |items| items[0].kept);
    assert_eq!(kept[0].key, "mate-terminal.desktop");
    d.press(&item, BTN_RIGHT);
    d.f.dispatch_until_mut(|f| d.probe.menu(f).iter().any(|l| l == "Remove from Dock"));

    // Close all windows asks each one to close.
    d.probe.send("choose Close all windows");
    d.f.dispatch_until_mut(|f| f.close_requested(first));
    d.f.dispatch_until_mut(|f| f.close_requested(second));
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn removing_a_kept_program_with_no_window_takes_its_item_away() {
    let mut d = Desk::new();
    d.probe.send("keep caja.desktop");
    let item = d.wait_for("caja.desktop", 0);

    d.press(&item, BTN_RIGHT);
    d.f.dispatch_until_mut(|f| !d.probe.menu(f).is_empty());
    assert_eq!(d.probe.menu(&mut d.f), ["Remove from Dock"]);
    d.probe.send("choose Remove from Dock");

    d.probe.wait_for_items(&mut d.f, |items| items.is_empty());
}

/// As an uninstall leaves it.
#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_kept_program_whose_entry_is_gone_can_be_removed() {
    let mut d = Desk::new();
    d.probe.send("keep uninstalled.desktop");
    let item = d.wait_for("uninstalled.desktop", 0);

    d.press(&item, BTN_RIGHT);
    d.f.dispatch_until_mut(|f| !d.probe.menu(f).is_empty());
    assert_eq!(d.probe.menu(&mut d.f), ["Remove from Dock"]);
    d.probe.send("choose Remove from Dock");

    d.probe.wait_for_items(&mut d.f, |items| items.is_empty());
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn keeping_removing_and_reordering_survive_the_panels_restart() {
    let mut d = Desk::new();
    for key in ["mate-terminal.desktop", "firefox.desktop", "caja.desktop"] {
        d.probe.send(&format!("keep {key}"));
    }
    let items = d.probe.wait_for_items(&mut d.f, |items| items.len() == 3);
    // Caja dragged to the front: dropped on the left half of the first item.
    let first = items[0].rect;
    d.probe.send(&format!(
        "drop caja.desktop {} {}",
        first.loc.x + 2,
        first.loc.y + first.size.h / 2
    ));
    let keys = |items: &[Item]| items.iter().map(|i| i.key.clone()).collect::<Vec<_>>();
    let items = d
        .probe
        .wait_for_items(&mut d.f, |items| items[0].key == "caja.desktop");
    assert_eq!(
        keys(&items),
        ["caja.desktop", "mate-terminal.desktop", "firefox.desktop"]
    );
    // And removed: Firefox.
    let firefox = items[2].clone();
    d.press(&firefox, BTN_RIGHT);
    d.f.dispatch_until_mut(|f| !d.probe.menu(f).is_empty());
    d.probe.send("choose Remove from Dock");
    let items = d.probe.wait_for_items(&mut d.f, |items| items.len() == 2);
    assert_eq!(keys(&items), ["caja.desktop", "mate-terminal.desktop"]);

    // The panel restarts: a second probe over the same settings.
    let Desk { mut f, probe } = d;
    let dir = probe.stop(&mut f);
    let mut probe = Probe::start(&mut f, dir, "bottom", 48, false);
    let items = probe.wait_for_items(&mut f, |items| items.len() == 2);
    assert_eq!(keys(&items), ["caja.desktop", "mate-terminal.desktop"]);
    assert!(items.iter().all(|item| item.kept && item.windows == 0));
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_vertical_dock_is_a_column_with_the_indicator_on_the_panels_edge() {
    let mut d = Desk::with("left", 40);
    d.probe.send("keep caja.desktop");
    let _terminal = d.open("mate-terminal", "Terminal");
    let _again = d.open("mate-terminal", "Terminal 2");
    let items = d
        .probe
        .wait_for_items(&mut d.f, |items| items.len() == 2 && items[1].windows == 2);

    assert_eq!(items[0].rect.loc.x, items[1].rect.loc.x, "one column");
    assert_eq!(items[1].rect.loc.y, items[0].rect.loc.y + 40);
    assert_eq!(items[0].rect.size, (40, 40).into());
    d.shot("05-vertical-dock");
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_launch_carries_an_activation_token_and_the_icon_pulses_until_the_first_window_maps() {
    let mut d = Desk::new();
    d.probe.send("keep mate-terminal.desktop");
    let item = d.wait_for("mate-terminal.desktop", 0);
    assert!(!item.pulsing, "nothing is launched");

    d.click(&item);

    d.wait_for_launches("mate-terminal", 1);
    assert!(
        !d.launch_token("mate-terminal").is_empty(),
        "the launch carries a token: {:?}",
        d.probe.launch_lines("mate-terminal")
    );
    let item = d.wait_for_item("mate-terminal.desktop", |item| item.pulsing);
    assert_eq!(item.windows, 0);
    assert!(!d.item("mate-terminal.desktop").highlighted);
    // Off the dock, so that neither the pointer nor the hover hides the pulse.
    d.f.pointer_move(f64::from(OUTPUT.0) / 2., f64::from(OUTPUT.1) / 2.);
    d.frames("07-launch-pulse", 4);

    let _terminal = d.open("mate-terminal", "Terminal");

    let item = d.wait_for("mate-terminal.desktop", 1);
    assert!(!item.pulsing, "the first window mapped");
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn the_token_a_launch_carries_gives_the_launched_programs_new_window_the_focus() {
    let mut d = Desk::new();
    let terminal = d.open_process(100, "mate-terminal", "Terminal");
    let files = d.open("caja", "Files");
    // A client that has a window of its own is refused the focus for a second one that
    // maps while another client has it, unless it comes with a recent token, which has to be
    // Caminito's: this is what the launch's token is for. A new instance of the program that
    // is running is the launch to try it with.
    d.focus_new_windows("smart", FocusNewWindows::Smart);
    let item = d.wait_for("mate-terminal.desktop", 1);

    d.press(&item, BTN_MIDDLE);

    d.wait_for_launches("mate-terminal", 1);
    let token = d.launch_token("mate-terminal");
    assert!(!token.is_empty(), "the launch carries a token");
    // In both variables: GTK 3 reads one, and other toolkits the other.
    let line = d.probe.launch_lines("mate-terminal").remove(0);
    assert!(line.ends_with(&format!("dsi={token}")), "{line}");
    d.wait_for_focus(files);
    let launched = d.map_launched(
        terminal.client(),
        "mate-terminal",
        "Terminal 2",
        Some(&token),
    );

    d.wait_for_focus(launched);
    assert!(!d.f.state.caminito.policy.is_urgent(d.f.pid(launched)));
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_new_window_that_maps_without_a_token_is_refused_the_focus() {
    let mut d = Desk::new();
    let terminal = d.open_process(100, "mate-terminal", "Terminal");
    let files = d.open("caja", "Files");
    d.focus_new_windows("smart", FocusNewWindows::Smart);

    let stranger = d.map_launched(terminal.client(), "mate-terminal", "Terminal 2", None);

    d.wait_for_focus(files);
    assert!(d.f.state.caminito.policy.is_urgent(d.f.pid(stranger)));
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn the_pulse_ends_after_the_timeout_for_a_program_that_maps_no_window() {
    let mut d = Desk::new();
    // The dock's ten seconds, shortened for the test.
    d.probe.send("launch-timeout 1500");
    d.probe.send("keep mate-terminal.desktop");
    let item = d.wait_for("mate-terminal.desktop", 0);

    d.click(&item);

    d.wait_for_launches("mate-terminal", 1);
    d.wait_for_item("mate-terminal.desktop", |item| item.pulsing);
    let item = d.wait_for_item("mate-terminal.desktop", |item| !item.pulsing);
    assert_eq!(item.windows, 0, "it never mapped a window");
    assert!(item.kept);
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_new_instance_of_a_running_program_pulses_until_its_own_window() {
    let mut d = Desk::new();
    let _terminal = d.open("mate-terminal", "Terminal");
    let item = d.wait_for("mate-terminal.desktop", 1);
    assert!(!item.pulsing, "a running program doesn't pulse");

    d.press(&item, BTN_MIDDLE);

    d.wait_for_launches("mate-terminal", 1);
    let item = d.wait_for_item("mate-terminal.desktop", |item| item.pulsing);
    assert_eq!(item.windows, 1, "its first window doesn't end it");
    let _second = d.open("mate-terminal", "Terminal 2");
    let item = d.wait_for("mate-terminal.desktop", 2);
    assert!(!item.pulsing);
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_window_that_demands_attention_bounces_then_keeps_a_highlight_until_it_is_focused() {
    let mut d = Desk::new();
    let terminal = d.open_with_input("mate-terminal", "Terminal");
    let _files = d.open("caja", "Files");
    d.strict();
    let item = d.item("mate-terminal.desktop");
    assert!(!item.bouncing && !item.highlighted);

    // The terminal asks to be activated with a token, and is refused the focus.
    let token = d.token_from(terminal);
    d.f.client(terminal.client())
        .activate(&token, terminal.xdg());
    d.f.roundtrip(terminal.client());

    let item = d.wait_for_item("mate-terminal.desktop", |item| item.highlighted);
    assert!(item.bouncing, "it bounces when the demand begins");
    d.frames("08-bounce", 6);
    // A few bounces, and then the highlight alone.
    let item = d.wait_for_item("mate-terminal.desktop", |item| !item.bouncing);
    assert!(item.highlighted);
    assert!(!d.item("caja.desktop").highlighted);
    d.shot("09-highlight");

    // From the dock.
    d.click(&item);

    d.wait_for_focus(terminal);
    let item = d.wait_for_item("mate-terminal.desktop", |item| !item.highlighted);
    assert!(!item.bouncing);
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn the_highlight_clears_when_the_window_is_focused_otherwise_than_from_the_dock() {
    let mut d = Desk::new();
    let terminal = d.open_with_input("mate-terminal", "Terminal");
    let _files = d.open("caja", "Files");
    d.strict();
    let token = d.token_from(terminal);
    d.f.client(terminal.client())
        .activate(&token, terminal.xdg());
    d.f.roundtrip(terminal.client());
    d.wait_for_item("mate-terminal.desktop", |item| item.highlighted);

    // A click on the window itself.
    click(&mut d.f, terminal);

    d.wait_for_focus(terminal);
    d.wait_for_item("mate-terminal.desktop", |item| {
        !item.highlighted && !item.bouncing
    });
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn only_the_item_whose_window_demands_attention_is_highlighted() {
    let mut d = Desk::new();
    let terminal = d.open_with_input("mate-terminal", "Terminal");
    let _files = d.open("caja", "Files");
    let _other = d.open("mystery-app", "Mystery");
    d.strict();
    let token = d.token_from(terminal);
    d.f.client(terminal.client())
        .activate(&token, terminal.xdg());
    d.f.roundtrip(terminal.client());

    let items = d
        .probe
        .wait_for_items(&mut d.f, |items| items.iter().any(|item| item.highlighted));

    let highlighted: Vec<_> = items
        .iter()
        .filter(|item| item.highlighted)
        .map(|item| item.key.as_str())
        .collect();
    assert_eq!(highlighted, ["mate-terminal.desktop"]);
}

/// A click on the middle of a window, as the user focuses it.
fn click(f: &mut Fixture, window: ClientWindow) {
    let geometry = f.geometry(window);
    f.click(
        f64::from(geometry.loc.x) + f64::from(geometry.size.w) / 2.,
        f64::from(geometry.loc.y) + f64::from(geometry.size.h) / 2.,
    );
    f.sync(window);
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn the_trash_is_at_the_end_of_the_row_after_a_separator_in_every_orientation() {
    for edge in ["bottom", "top", "left", "right"] {
        let mut d = Desk::with(edge, 40);
        let _terminal = d.open("mate-terminal", "Terminal");
        let last = d.wait_for("mate-terminal.desktop", 1);
        let trash = d.trash();
        let horizontal = edge == "bottom" || edge == "top";

        assert_eq!(trash.rect.size, (40, 40).into(), "{edge}: an item's size");
        let (last_end, trash_start, across_last, across_trash) = if horizontal {
            (
                last.rect.loc.x + last.rect.size.w,
                trash.rect.loc.x,
                last.rect.loc.y,
                trash.rect.loc.y,
            )
        } else {
            (
                last.rect.loc.y + last.rect.size.h,
                trash.rect.loc.y,
                last.rect.loc.x,
                trash.rect.loc.x,
            )
        };
        assert_eq!(across_last, across_trash, "{edge}: in the row of the items");
        assert!(
            trash_start >= last_end + 8,
            "{edge}: a separator between the last item at {last_end} and the Trash at \
             {trash_start}"
        );
    }
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe, and gvfs"]
fn dropping_a_file_on_the_trash_moves_it_there_and_the_icon_turns_full() {
    let mut d = Desk::with_gvfs();
    let _terminal = d.open("mate-terminal", "Terminal");
    d.wait_for_trash(false);
    d.shot("07-trash-empty");

    let (path, uri) = d.probe.file("report.txt");
    d.probe.send(&format!("drop-files {uri}"));
    d.wait_for_trash(true);
    assert!(!path.exists(), "the file left its place");
    assert_eq!(d.probe.trashed(), ["report.txt"], "and is in the Trash");
    d.shot("08-trash-full");

    // The icon follows the Trash as it changes by other hands too: Caja emptying it.
    std::fs::remove_file(d.probe.dir.path().join("data/Trash/files/report.txt")).unwrap();
    d.wait_for_trash(false);
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe, and gvfs"]
fn a_file_that_cannot_be_trashed_is_reported_and_the_others_still_go() {
    let mut d = Desk::with_gvfs();
    d.wait_for_trash(false);
    let missing = d.probe.dir.path().join("outside/missing.txt");
    let (good, uri) = d.probe.file("good.txt");
    d.probe
        .send(&format!("drop-files file://{} {uri}", missing.display()));

    d.wait_for_trash(true);
    assert!(!good.exists(), "the file that can go went");
    let probe = &d.probe;
    d.f.dispatch_until_explained(
        |_| probe.log().contains("can't move file://"),
        || format!("the failure wasn't reported: {}", probe.log()),
    );
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe, and gvfs"]
fn a_click_on_the_trash_opens_it_in_caja() {
    let mut d = Desk::with_gvfs();
    let trash = d.trash();

    d.press_rect(trash.rect, BTN_LEFT);
    let probe = &d.probe;
    d.f.dispatch_until_explained(
        |_| !probe.opened_in_caja().is_empty(),
        || format!("Caja wasn't asked to open the Trash: {}", probe.log()),
    );
    assert_eq!(d.probe.opened_in_caja(), ["trash:///"]);
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe, and gvfs"]
fn the_trashs_menu_is_open_and_empty_trash_which_is_only_for_a_full_trash() {
    let mut d = Desk::with_gvfs();
    let trash = d.wait_for_trash(false);

    d.press_rect(trash.rect, BTN_RIGHT);
    assert_eq!(
        d.wait_for_menu(),
        ["Open", "Empty Trash"].map(str::to_owned)
    );
    assert_eq!(d.probe.menu_disabled(&mut d.f), ["Empty Trash"]);
    d.probe.send("choose Open");
    let probe = &d.probe;
    d.f.dispatch_until_explained(
        |_| !probe.opened_in_caja().is_empty(),
        || format!("Open didn't open the Trash: {}", probe.log()),
    );

    let (_, uri) = d.probe.file("a.txt");
    d.probe.send(&format!("drop-files {uri}"));
    d.wait_for_trash(true);
    d.press_rect(trash.rect, BTN_RIGHT);
    d.wait_for_menu();
    assert!(d.probe.menu_disabled(&mut d.f).is_empty());
    d.shot("09-trash-menu");
}

/// A full Trash: a file and a folder with a file in it.
fn fill_the_trash(d: &mut Desk) {
    let (_, one) = d.probe.file("one.txt");
    let (folder, _) = d.probe.file("folder/inside.txt");
    d.probe.send(&format!(
        "drop-files {one} file://{}",
        folder.parent().unwrap().display()
    ));
    d.wait_for_trash(true);
    let probe = &d.probe;
    d.f.dispatch_until_explained(
        |_| probe.trashed() == ["folder", "one.txt"],
        || format!("the Trash holds {:?}: {}", probe.trashed(), probe.log()),
    );
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe, and gvfs"]
fn emptying_the_trash_asks_first_and_confirmed_empties_it() {
    let mut d = Desk::with_gvfs();
    fill_the_trash(&mut d);
    let trash = d.trash();

    d.press_rect(trash.rect, BTN_RIGHT);
    d.wait_for_menu();
    d.probe.send("choose Empty Trash");
    d.f.dispatch_until_mut(|f| !d.probe.dialog(f).is_empty());
    assert_eq!(
        d.probe.dialog(&mut d.f),
        ["Empty all items from Trash?"],
        "it asks, as Caja does"
    );
    assert_eq!(
        d.probe.trashed(),
        ["folder", "one.txt"],
        "and has emptied nothing yet"
    );
    d.shot("10-trash-confirmation");

    d.probe.send("respond accept");
    d.wait_for_trash(false);
    assert_eq!(
        d.probe.trashed(),
        Vec::<String>::new(),
        "confirmed, it is empty"
    );
    assert!(
        d.probe.dialog(&mut d.f).is_empty(),
        "and the question is gone"
    );
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe, and gvfs"]
fn cancelling_the_confirmation_leaves_the_trash_as_it_was() {
    let mut d = Desk::with_gvfs();
    fill_the_trash(&mut d);
    let trash = d.trash();

    d.press_rect(trash.rect, BTN_RIGHT);
    d.wait_for_menu();
    d.probe.send("choose Empty Trash");
    d.f.dispatch_until_mut(|f| !d.probe.dialog(f).is_empty());
    d.probe.send("respond cancel");
    d.f.dispatch_until_mut(|f| d.probe.dialog(f).is_empty());
    // Nothing is to happen: an emptying, had cancelling started one, would have deleted the
    // two small files and told the monitor well within half a second.
    d.f.dispatch_for(Duration::from_millis(500));

    assert_eq!(d.probe.trashed(), ["folder", "one.txt"]);
    assert!(d.probe.trash(&mut d.f).full, "the icon is still full");
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn two_windows_of_one_program_with_one_title_get_their_own_identifiers_and_ext_handles() {
    let mut d = Desk::new();
    let first = d.open("mate-terminal", "Terminal");
    let second = d.open("mate-terminal", "Terminal");

    let named = d.probe.identifiers(&mut d.f);

    assert_eq!(named.len(), 2, "{named:?}");
    let caminito = &d.f.state.caminito;
    let mut expected = [first, second].map(|window| caminito.toplevel_identifier(d.f.pid(window)));
    let mut found: Vec<_> = named
        .iter()
        .map(|(title, identifier, ext)| {
            assert_eq!(title, "Terminal");
            assert!(ext, "the ext handle with {identifier:?} is found");
            identifier.clone()
        })
        .collect();
    assert_ne!(found[0], found[1], "one identifier for two windows");
    // Each window has the identifier the ext list gives it, whatever order they are listed in.
    expected.sort();
    found.sort();
    assert_eq!(found, expected);
}

fn uris(folders: &[Folder]) -> Vec<String> {
    folders.iter().map(|folder| folder.uri.clone()).collect()
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_kept_folder_is_between_the_separator_and_the_trash_in_every_orientation() {
    for edge in ["bottom", "top", "left", "right"] {
        let mut d = Desk::with(edge, 40);
        let _terminal = d.open("mate-terminal", "Terminal");
        let last = d.wait_for("mate-terminal.desktop", 1);
        let (_, uri) = d.probe.folder("Documents");
        d.probe.send(&format!("keep-folders 0 {uri}"));
        let folders = d.wait_for_folders(|folders| folders.len() == 1);
        let trash = d.trash();
        let horizontal = edge == "bottom" || edge == "top";
        let (last_end, folder_start, folder_end, trash_start) = if horizontal {
            (
                last.rect.loc.x + last.rect.size.w,
                folders[0].rect.loc.x,
                folders[0].rect.loc.x + folders[0].rect.size.w,
                trash.rect.loc.x,
            )
        } else {
            (
                last.rect.loc.y + last.rect.size.h,
                folders[0].rect.loc.y,
                folders[0].rect.loc.y + folders[0].rect.size.h,
                trash.rect.loc.y,
            )
        };

        assert_eq!(
            folders[0].rect.size,
            (40, 40).into(),
            "{edge}: an item's size"
        );
        assert!(folder_start >= last_end + 8, "{edge}: after the separator");
        assert_eq!(
            folder_end, trash_start,
            "{edge}: and right before the Trash"
        );
        if edge == "bottom" {
            d.shot("10-kept-folder");
        }
        if edge == "left" {
            d.shot("14-kept-folder-vertical");
        }
    }
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_kept_folder_survives_the_panels_restart_and_remove_from_dock_takes_it_away() {
    let mut d = Desk::new();
    let (_, music) = d.probe.folder("Music");
    let (_, documents) = d.probe.folder("Documents");
    // A file is not a folder, so it is not kept.
    let (_, file) = d.probe.file("report.txt");
    d.probe
        .send(&format!("keep-folders 0 {music} {file} {documents}"));
    let folders = d.wait_for_folders(|folders| folders.len() == 2);
    assert_eq!(uris(&folders), [music.clone(), documents.clone()]);

    // The panel restarts: a second probe over the same settings.
    let Desk { mut f, probe } = d;
    let dir = probe.stop(&mut f);
    let probe = Probe::start(&mut f, dir, "bottom", 48, false);
    let mut d = Desk { f, probe };
    let folders = d.wait_for_folders(|folders| folders.len() == 2);
    assert_eq!(uris(&folders), [music, documents.clone()]);

    d.press_rect(folders[1].rect, BTN_RIGHT);
    assert_eq!(
        d.wait_for_menu(),
        ["Open in Caja", "Remove from Dock"].map(str::to_owned)
    );
    d.shot("12-kept-folder-menu");
    d.probe.send("choose Remove from Dock");
    let folders = d.wait_for_folders(|folders| folders.len() == 1);
    assert!(!uris(&folders).contains(&documents));
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn kept_folders_are_reordered_among_themselves_and_never_before_the_separator() {
    let mut d = Desk::new();
    let _terminal = d.open("mate-terminal", "Terminal");
    d.wait_for("mate-terminal.desktop", 1);
    let uri = |name: &str| d.probe.folder(name).1;
    let (a, b, c) = (uri("A"), uri("B"), uri("C"));
    d.probe.send(&format!("keep-folders 0 {a} {b} {c}"));
    d.wait_for_folders(|folders| folders.len() == 3);

    d.probe.send(&format!("move-folder {c} 0"));
    let folders = d.wait_for_folders(|folders| folders.first().is_some_and(|f| f.uri == c));
    assert_eq!(uris(&folders), [c.clone(), a.clone(), b.clone()]);
    // A position past the end is the end, and the Dock items stay before the separator.
    d.probe.send(&format!("move-folder {c} 99"));
    let folders = d.wait_for_folders(|folders| folders.last().is_some_and(|f| f.uri == c));
    assert_eq!(uris(&folders), [a, b, c]);
    let item = d.item("mate-terminal.desktop");
    assert!(
        folders
            .iter()
            .all(|folder| folder.rect.loc.x > item.rect.loc.x)
    );
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_click_on_a_kept_folder_shows_a_grid_of_its_contents_folders_first() {
    let mut d = Desk::new();
    let (path, uri) = d.probe.folder("Documents");
    std::fs::write(path.join("b.txt"), "b").unwrap();
    std::fs::write(path.join("A.txt"), "a").unwrap();
    std::fs::create_dir(path.join("zeta")).unwrap();
    d.probe.send(&format!("keep-folders 0 {uri}"));
    let folders = d.wait_for_folders(|folders| folders.len() == 1);

    d.press_rect(folders[0].rect, BTN_LEFT);
    d.f.dispatch_until_mut(|f| !d.probe.grid(f).is_empty());
    assert_eq!(
        d.probe.grid(&mut d.f),
        ["zeta/", "A.txt", "b.txt", "inside.txt"]
    );
    d.shot("11-kept-folder-grid");
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_file_dropped_on_a_kept_folder_is_moved_into_it_on_the_same_file_system() {
    let mut d = Desk::new();
    let (folder, uri) = d.probe.folder("Documents");
    d.probe.send(&format!("keep-folders 0 {uri}"));
    d.wait_for_folders(|folders| folders.len() == 1);

    let (path, file) = d.probe.file("report.txt");
    d.probe.send(&format!("drop-on-folder {uri} {file}"));
    d.f.dispatch_until(|_| folder.join("report.txt").exists());
    assert!(!path.exists(), "the file left its place");
}

#[test]
#[ignore = "needs CAMINITO_DOCK_PROBE, the built dk-probe"]
fn a_kept_folder_that_is_gone_is_dimmed_says_so_on_a_click_and_comes_back_with_the_folder() {
    let mut d = Desk::new();
    let (path, uri) = d.probe.folder("Documents");
    d.probe.send(&format!("keep-folders 0 {uri}"));
    d.wait_for_folders(|folders| folders.len() == 1 && !folders[0].gone);

    std::fs::remove_dir_all(&path).unwrap();
    let folders = d.wait_for_folders(|folders| folders.len() == 1 && folders[0].gone);
    d.press_rect(folders[0].rect, BTN_LEFT);
    d.f.dispatch_until_mut(|f| !d.probe.error(f).is_empty());
    assert_eq!(d.probe.error(&mut d.f), ["“Documents” isn't available"]);
    d.shot("13-kept-folder-gone");

    std::fs::create_dir(&path).unwrap();
    d.wait_for_folders(|folders| folders.len() == 1 && !folders[0].gone);
}
