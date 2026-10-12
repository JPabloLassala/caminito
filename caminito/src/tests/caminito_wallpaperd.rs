//! caminito-wallpaperd against Caminito (210, ADR 0031): the real drawer puts one
//! wallpaper surface on every output, in the namespace the scene puts behind caja's desktop,
//! follows the outputs as they come and go without leaving any behind (88), draws a change
//! of its settings without a restart, takes MATE's picture on its first run alone, and keeps
//! `org.mate.background` bridged both ways (211).
//!
//! It needs the built drawer, named by `CAMINITO_WALLPAPERD`, so these are ignored by
//! default and `tools/check` runs them after the workspace's build. As the locker's tests
//! do (`caminito_locker`), the drawer connects to a socket of the test's, whose connection
//! the test hands to Caminito, and its GSettings are a keyfile in the test's directory, on
//! the test schemas, so a test can set keys and change them. It has no session bus, so it
//! takes the first output for the primary one.

use std::io::ErrorKind;
use std::os::unix::net::UnixListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use smithay::output::Output;

use super::fixture::{Fixture, test_schemas};

/// The namespace of the drawer's surfaces, and msd's before it.
const NAMESPACE: &str = "mate-background";

/// Every output's wallpaper is the colour `0x608040`, and the first run is over.
const GREEN: &str = "\
[org/mate/caminito/wallpaper]
took-mate-background=true

[org/mate/caminito/wallpaper/all]
kind='colour'
colour='#608040'
";

/// caminito-wallpaperd in a scratch directory with its socket, its settings and its log.
struct Drawer {
    dir: tempfile::TempDir,
    listener: UnixListener,
    child: Option<Child>,
}

impl Drawer {
    /// A directory for the drawer, with its GSettings set to `keyfile`. It isn't started.
    fn new(keyfile: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let listener = UnixListener::bind(dir.path().join("wayland-test")).unwrap();
        listener.set_nonblocking(true).unwrap();
        let drawer = Drawer {
            dir,
            listener,
            child: None,
        };
        drawer.set_settings(keyfile);
        drawer
    }

    /// A drawer started on `f`'s Caminito with its GSettings set to `keyfile`.
    fn start_with(f: &mut Fixture, keyfile: &str) -> Self {
        let mut drawer = Drawer::new(keyfile);
        drawer.start(f);
        drawer
    }

    fn path(&self, name: &str) -> PathBuf {
        self.dir.path().join(name)
    }

    /// Starts caminito-wallpaperd, and hands its connection to Caminito as its process's.
    fn start(&mut self, f: &mut Fixture) {
        let program = std::env::var_os("CAMINITO_WALLPAPERD")
            .expect("CAMINITO_WALLPAPERD names the built caminito-wallpaperd");
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(self.path("log"))
            .unwrap();
        let child = Command::new(program)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("XDG_RUNTIME_DIR", self.dir.path())
            .env("WAYLAND_DISPLAY", "wayland-test")
            .env("GSETTINGS_BACKEND", "keyfile")
            .env("XDG_CONFIG_HOME", self.path("config"))
            .env("GSETTINGS_SCHEMA_DIR", test_schemas())
            .env("GIO_USE_VFS", "local")
            .env("DBUS_SESSION_BUS_ADDRESS", "unix:path=/nonexistent")
            .env("G_MESSAGES_DEBUG", "caminito-wallpaperd")
            .stdin(Stdio::null())
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .unwrap();
        let pid = child.id();
        self.child = Some(child);
        f.dispatch_until_mut(|f| match self.listener.accept() {
            Ok((stream, _)) => {
                f.insert_stream(stream, Some(pid));
                true
            }
            Err(err) if err.kind() == ErrorKind::WouldBlock => false,
            Err(err) => panic!("can't accept caminito-wallpaperd's connection: {err}"),
        });
    }

    /// Kills the drawer, as `killall -9` would, and waits for it.
    fn kill(&mut self) {
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn keyfile(&self) -> PathBuf {
        self.path("config/glib-2.0/settings/keyfile")
    }

    /// Sets the drawer's GSettings to `keyfile`, before it starts or while it runs.
    fn set_settings(&self, keyfile: &str) {
        std::fs::create_dir_all(self.keyfile().parent().unwrap()).unwrap();
        // Written whole and renamed, so the drawer never reads half of it.
        let partial = self.path("keyfile.partial");
        std::fs::write(&partial, keyfile).unwrap();
        std::fs::rename(partial, self.keyfile()).unwrap();
    }

    /// The drawer's GSettings as it last wrote them.
    fn settings(&self) -> String {
        std::fs::read_to_string(self.keyfile()).unwrap_or_default()
    }

    /// What the drawer wrote so far.
    fn seen(&self) -> String {
        std::fs::read_to_string(self.path("log")).unwrap_or_default()
    }
}

impl Drop for Drawer {
    fn drop(&mut self) {
        self.kill();
    }
}

/// How many of the drawer's surfaces each output has, by its name, from the dump.
fn wallpapers(f: &Fixture) -> Vec<(String, usize)> {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let outputs = dump["outputs"].as_array().unwrap();
    let count = |output: &serde_json::Value| {
        let surfaces = output["layer_surfaces"].as_array().unwrap();
        let wallpaper = |surface: &&serde_json::Value| {
            surface["namespace"] == NAMESPACE && surface["layer"] == "background"
        };
        let name = output["name"].as_str().unwrap().to_owned();
        (name, surfaces.iter().filter(wallpaper).count())
    };
    outputs.iter().map(count).collect()
}

/// Each output's name with one wallpaper surface.
fn one_each(names: &[&str]) -> Vec<(String, usize)> {
    names.iter().map(|name| ((*name).to_owned(), 1)).collect()
}

/// Waits until every output in `outputs` shows `color` in its middle and in three corners,
/// in logical points. The fourth, the top left, has the pointer at the origin.
fn wait_for_color(f: &mut Fixture, outputs: &[Output], color: u32) {
    for output in outputs {
        let scale = output.current_scale().fractional_scale();
        let size = output
            .current_mode()
            .unwrap()
            .size
            .to_f64()
            .to_logical(scale);
        let (w, h) = (size.w as i32, size.h as i32);
        for (x, y) in [(w / 2, h / 2), (w - 2, 1), (1, h - 2), (w - 2, h - 2)] {
            f.wait_for_pixel_on(output, x, y, color);
        }
    }
}

#[test]
#[ignore = "needs CAMINITO_WALLPAPERD; tools/check runs it"]
fn each_output_has_one_wallpaper_as_outputs_come_and_go() {
    let mut f = Fixture::new();
    let first = f.add_output((1920, 1080), 1.0);
    let second = f.add_output((1280, 1024), 1.0);
    let drawer = Drawer::start_with(&mut f, GREEN);
    let seen = || drawer.seen();

    f.dispatch_until_explained(
        |f| wallpapers(f) == one_each(&["headless-1", "headless-2"]),
        seen,
    );
    wait_for_color(&mut f, &[first.clone(), second.clone()], 0x608040);

    // A second output added gets one.
    let third = f.add_output((800, 600), 2.0);
    f.dispatch_until_explained(
        |f| wallpapers(f) == one_each(&["headless-1", "headless-2", "headless-3"]),
        seen,
    );
    wait_for_color(&mut f, &[third], 0x608040);

    // One unplugged takes its surface with it, and the others keep theirs alone.
    f.unplug(&second);
    f.dispatch_until_explained(
        |f| wallpapers(f) == one_each(&["headless-1", "headless-3"]),
        seen,
    );
    // Plugged back, it has one again, and no output has more (88).
    let second = f.replug(&second);
    f.dispatch_until_explained(
        |f| wallpapers(f) == one_each(&["headless-1", "headless-3", "headless-2"]),
        seen,
    );
    wait_for_color(&mut f, &[first, second], 0x608040);
    // Nothing more is to come: the drawer's surfaces stay one an output. A pile-up, as msd's
    // was, came within a dispatch or two of the hot-plug.
    f.dispatch_for(std::time::Duration::from_millis(300));
    assert_eq!(
        wallpapers(&f),
        one_each(&["headless-1", "headless-3", "headless-2"])
    );
}

#[test]
#[ignore = "needs CAMINITO_WALLPAPERD; tools/check runs it"]
fn a_change_of_a_key_is_drawn_without_a_restart() {
    let mut f = Fixture::new();
    let outputs = [
        f.add_output((1920, 1080), 1.0),
        f.add_output((1280, 1024), 1.0),
    ];
    let mut drawer = Drawer::start_with(&mut f, GREEN);
    wait_for_color(&mut f, &outputs, 0x608040);

    drawer.set_settings(&GREEN.replace("#608040", "#406080"));

    // After the cross-fade, which is about a second, from the drawer that drew the first.
    wait_for_color(&mut f, &outputs, 0x406080);
    let child = drawer.child.as_mut().unwrap();
    assert!(child.try_wait().unwrap().is_none(), "{}", drawer.seen());
}

/// MATE's background, the picture `file` filling the output, over black.
fn mate_background(file: &Path) -> String {
    format!(
        "[org/mate/desktop/background]\n\
         picture-filename='{}'\n\
         picture-options='zoom'\n\
         primary-color='#000000'\n",
        file.display()
    )
}

#[test]
#[ignore = "needs CAMINITO_WALLPAPERD; tools/check runs it"]
fn the_first_run_takes_mates_picture_and_the_second_doesnt() {
    let mut f = Fixture::new();
    let outputs = [
        f.add_output((1920, 1080), 1.0),
        f.add_output((1280, 1024), 1.0),
    ];
    let pictures = tempfile::tempdir().unwrap();
    let first = pictures.path().join("first.png");
    let later = pictures.path().join("later.png");
    std::fs::write(&first, png(0x804060)).unwrap();
    std::fs::write(&later, png(0x206040)).unwrap();
    let mut drawer = Drawer::start_with(&mut f, &mate_background(&first));
    let seen = || drawer.seen();

    wait_for_color(&mut f, &outputs, 0x804060);
    f.dispatch_until_explained(
        |_| drawer.settings().contains("took-mate-background=true"),
        seen,
    );

    // MATE's picture changes while the drawer is away, and the second run doesn't take it.
    drawer.kill();
    f.dispatch_until(|f| wallpapers(f).iter().all(|(_, count)| *count == 0));
    let settings = drawer.settings();
    let mate = |file: &Path| format!("picture-filename='{}'", file.display());
    assert!(settings.contains(&mate(&first)), "{settings}");
    drawer.set_settings(&settings.replace(&mate(&first), &mate(&later)));
    drawer.start(&mut f);
    f.dispatch_until_explained(
        |f| wallpapers(f) == one_each(&["headless-1", "headless-2"]),
        || drawer.seen(),
    );
    wait_for_color(&mut f, &outputs, 0x804060);
}

/// The two headless outputs, each with a colour of its own, `0x608040` the first's and
/// `0x406080` the second's, "same on all outputs" off.
const EACH_ITS_OWN: &str = "\
[org/mate/caminito/wallpaper]
took-mate-background=true
same-on-all-outputs=false
outputs=['connector-headless_2D1', 'connector-headless_2D2']

[org/mate/caminito/wallpaper/outputs/connector-headless_2D1]
kind='colour'
colour='#608040'

[org/mate/caminito/wallpaper/outputs/connector-headless_2D2]
kind='colour'
colour='#406080'
";

/// `settings` with the key `key` of the group `group` set to `value`, as GVariant text.
fn with_key(settings: &str, group: &str, key: &str, value: &str) -> String {
    let mut lines: Vec<String> = settings.lines().map(str::to_owned).collect();
    let header = format!("[{group}]");
    let start = match lines.iter().position(|line| *line == header) {
        Some(start) => start,
        None => {
            lines.extend([String::new(), header]);
            lines.len() - 1
        }
    };
    let end = lines[start + 1..]
        .iter()
        .position(|line| line.starts_with('['))
        .map_or(lines.len(), |end| start + 1 + end);
    let prefix = format!("{key}=");
    let line = format!("{key}={value}");
    match (start + 1..end).find(|&index| lines[index].starts_with(&prefix)) {
        Some(index) => lines[index] = line,
        None => lines.insert(end, line),
    }
    lines.join("\n") + "\n"
}

/// The group of MATE's keys in a keyfile.
const MATE: &str = "org/mate/desktop/background";

#[test]
#[ignore = "needs CAMINITO_WALLPAPERD; tools/check runs it"]
fn mates_picture_follows_the_primary_output() {
    let mut f = Fixture::new();
    let outputs = [
        f.add_output((1920, 1080), 1.0),
        f.add_output((1280, 1024), 1.0),
    ];
    let pictures = tempfile::tempdir().unwrap();
    let file = pictures.path().join("primary.png");
    std::fs::write(&file, png(0x804060)).unwrap();
    let drawer = Drawer::start_with(&mut f, EACH_ITS_OWN);
    let seen = || format!("{}\n{}", drawer.settings(), drawer.seen());

    // A colour goes out as its colour and no picture.
    wait_for_color(&mut f, &outputs[..1], 0x608040);
    f.dispatch_until_explained(
        |_| {
            let settings = drawer.settings();
            settings.contains("primary-color='#608040'")
                && settings.contains("picture-filename=''")
                && settings.contains("color-shading-type='solid'")
        },
        seen,
    );

    // The primary output's picture goes out, with its positioning.
    let primary = "org/mate/caminito/wallpaper/outputs/connector-headless_2D1";
    let settings = with_key(&drawer.settings(), primary, "kind", "'picture'");
    let settings = with_key(
        &settings,
        primary,
        "picture",
        &format!("'{}'", file.display()),
    );
    drawer.set_settings(&with_key(&settings, primary, "positioning", "'tile'"));
    let named = format!("picture-filename='{}'", file.display());
    f.dispatch_until_explained(
        |_| {
            let settings = drawer.settings();
            settings.contains(&named) && settings.contains("picture-options='wallpaper'")
        },
        seen,
    );
    // The drawer's own write isn't taken for another's: the second output keeps its colour.
    wait_for_color(&mut f, &outputs[..1], 0x804060);
    // Taken for another's, it would have started the second output's fade by the first's
    // end, and a fade is under way within a frame.
    f.dispatch_for(std::time::Duration::from_millis(300));
    wait_for_color(&mut f, &outputs[1..], 0x406080);
}

#[test]
#[ignore = "needs CAMINITO_WALLPAPERD; tools/check runs it"]
fn a_picture_another_program_gives_mate_is_every_outputs() {
    let mut f = Fixture::new();
    let outputs = [
        f.add_output((1920, 1080), 1.0),
        f.add_output((1280, 1024), 1.0),
    ];
    let pictures = tempfile::tempdir().unwrap();
    let file = pictures.path().join("caja.png");
    std::fs::write(&file, png(0x804060)).unwrap();
    let drawer = Drawer::start_with(&mut f, EACH_ITS_OWN);
    let seen = || format!("{}\n{}", drawer.settings(), drawer.seen());
    wait_for_color(&mut f, &outputs[..1], 0x608040);
    wait_for_color(&mut f, &outputs[1..], 0x406080);
    f.dispatch_until_explained(
        |_| drawer.settings().contains("primary-color='#608040'"),
        seen,
    );

    // As caja's "Set as wallpaper" does.
    drawer.set_settings(&with_key(
        &drawer.settings(),
        MATE,
        "picture-filename",
        &format!("'{}'", file.display()),
    ));

    wait_for_color(&mut f, &outputs, 0x804060);
    let settings = drawer.settings();
    let set = format!("picture='{}'", file.display());
    assert_eq!(settings.matches(&set).count(), 3, "{settings}");
}

/// A PNG of 8 by 8 pixels of `rgb`, `0xrrggbb`, uncompressed.
fn png(rgb: u32) -> Vec<u8> {
    const SIZE: u32 = 8;
    fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
        png.extend((data.len() as u32).to_be_bytes());
        let start = png.len();
        png.extend(kind);
        png.extend(data);
        let crc = crc32(&png[start..]);
        png.extend(crc.to_be_bytes());
    }
    let pixel = [(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8];
    // Each row is its filter, none, and its pixels.
    let mut row = vec![0];
    row.extend(pixel.repeat(SIZE as usize));
    let rows = row.repeat(SIZE as usize);
    // zlib with one stored block.
    let mut zlib = vec![0x78, 0x01, 0x01];
    zlib.extend((rows.len() as u16).to_le_bytes());
    zlib.extend((!(rows.len() as u16)).to_le_bytes());
    zlib.extend(&rows);
    zlib.extend(adler32(&rows).to_be_bytes());

    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut header = Vec::new();
    header.extend(SIZE.to_be_bytes());
    header.extend(SIZE.to_be_bytes());
    // 8 bits a channel, RGB, deflate, no filter method, no interlace.
    header.extend([8, 2, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &header);
    chunk(&mut png, b"IDAT", &zlib);
    chunk(&mut png, b"IEND", &[]);
    png
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &byte in bytes {
        a = (a + u32::from(byte)) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}
