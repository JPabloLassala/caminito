//! `caminito-gtk-palette` itself (28), run for the fixed GTK theme.
//!
//! These need the helper built by Meson, named by `CAMINITO_GTK_PALETTE`, and an X display,
//! so they're ignored by default. CI runs them under `xvfb-run` with `GDK_BACKEND=x11`:
//! the colours don't depend on the GDK backend.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use caminito_theme::{
    GtkState, PaletteColors, PaletteReader, ReportedPalette, Rgba, StateColors, names_request,
};
use serde_json::Value;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// A private home with the vendored schemas and the keyfile backend, so `gsettings` in
/// another process changes what the helper sees. Nothing of the machine's own GTK or
/// GSettings configuration reaches it.
struct Environment {
    dir: tempfile::TempDir,
}

impl Environment {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let schemas = dir.path().join("schemas");
        std::fs::create_dir(&schemas).unwrap();
        let status = Command::new("glib-compile-schemas")
            .arg(format!("--targetdir={}", schemas.display()))
            .arg(repo().join("tests/schemas"))
            .status()
            .expect("glib-compile-schemas runs");
        assert!(status.success());
        Environment { dir }
    }

    fn command(&self, program: impl AsRef<std::ffi::OsStr>) -> Command {
        let home = self.dir.path().join("home");
        let mut command = Command::new(program);
        command
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .env("XDG_DATA_HOME", home.join(".local/share"))
            .env("XDG_DATA_DIRS", self.dir.path().join("no-data"))
            .env("GSETTINGS_SCHEMA_DIR", self.dir.path().join("schemas"))
            .env("GSETTINGS_BACKEND", "keyfile")
            .env("GDK_BACKEND", "x11");
        for name in ["DISPLAY", "XAUTHORITY"] {
            if let Some(value) = std::env::var_os(name) {
                command.env(name, value);
            }
        }
        command
    }

    fn set_gtk_theme(&self, theme: &str) {
        let status = self
            .command("gsettings")
            .args(["set", "org.mate.interface", "gtk-theme", theme])
            .status()
            .expect("gsettings runs");
        assert!(status.success());
    }

    fn spawn_helper(&self) -> Helper {
        let program = std::env::var_os("CAMINITO_GTK_PALETTE")
            .expect("CAMINITO_GTK_PALETTE names the built caminito-gtk-palette");
        let mut child = self
            .command(program)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("caminito-gtk-palette starts");
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, palettes) = mpsc::channel();
        std::thread::spawn(move || {
            let mut reader = PaletteReader::new();
            for line in BufReader::new(stdout).split(b'\n') {
                let mut line = line.unwrap();
                line.push(b'\n');
                for palette in reader.feed(&line) {
                    if sender
                        .send(palette.expect("a well-formed palette"))
                        .is_err()
                    {
                        return;
                    }
                }
            }
        });
        Helper {
            child,
            stdin,
            palettes,
        }
    }
}

struct Helper {
    child: Child,
    stdin: ChildStdin,
    palettes: mpsc::Receiver<ReportedPalette>,
}

impl Helper {
    fn ask_for(&mut self, names: &[String]) {
        let request = names_request(names.iter().map(String::as_str));
        self.stdin.write_all(request.as_bytes()).unwrap();
    }

    fn next_palette(&self) -> ReportedPalette {
        self.palettes
            .recv_timeout(Duration::from_secs(20))
            .expect("the helper reports a palette")
    }
}

impl Drop for Helper {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn goldens_palette() -> Value {
    let path = repo().join("tests/goldens/palette.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

fn rgba(value: &Value) -> Option<Rgba> {
    let channels: Vec<f64> = value
        .as_array()?
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    Some(Rgba {
        red: channels[0],
        green: channels[1],
        blue: channels[2],
        alpha: channels[3],
    })
}

/// A variant of the palette `marco-frame-dump` recorded through Marco's own renderer. A
/// `null` name is one the GTK theme doesn't define, which the helper leaves out.
fn recorded_colors(variant: &Value) -> PaletteColors {
    let states = GtkState::ALL.map(|state| {
        let colors = &variant["states"][state.name().to_ascii_lowercase()];
        StateColors {
            background: rgba(&colors["background"]).unwrap(),
            text: rgba(&colors["text"]).unwrap(),
        }
    });
    let named: BTreeMap<String, Rgba> = variant["named"]
        .as_object()
        .unwrap()
        .iter()
        .filter_map(|(name, value)| Some((name.clone(), rgba(value)?)))
        .collect();
    PaletteColors::new(states, named)
}

fn recorded_names(recorded: &Value) -> Vec<String> {
    recorded["variants"]["default"]["named"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

#[test]
#[ignore = "needs CAMINITO_GTK_PALETTE and an X display; CI runs it under xvfb-run"]
fn the_helper_reports_the_colours_marco_renders() {
    let recorded = goldens_palette();
    let gtk_theme = recorded["gtk_theme"].as_str().unwrap();
    let environment = Environment::new();
    environment.set_gtk_theme(gtk_theme);

    let mut helper = environment.spawn_helper();
    helper.ask_for(&recorded_names(&recorded));
    let reported = helper.next_palette();

    assert_eq!(reported.gtk_theme, gtk_theme);
    assert_eq!(
        reported.palette.default,
        recorded_colors(&recorded["variants"]["default"]),
        "default variant"
    );
    assert_eq!(
        reported.palette.dark,
        recorded_colors(&recorded["variants"]["dark"]),
        "dark variant"
    );
}

#[test]
#[ignore = "needs CAMINITO_GTK_PALETTE and an X display; CI runs it under xvfb-run"]
fn changing_gtk_theme_reports_the_new_themes_palette() {
    let environment = Environment::new();
    environment.set_gtk_theme("Adwaita");
    let mut helper = environment.spawn_helper();
    helper.ask_for(&["wm_bg_a".to_owned()]);
    let adwaita = helper.next_palette();
    assert_eq!(adwaita.gtk_theme, "Adwaita");

    // GTK3's other built-in theme.
    environment.set_gtk_theme("HighContrast");
    let high_contrast = helper.next_palette();
    assert_eq!(high_contrast.gtk_theme, "HighContrast");
    assert_ne!(
        high_contrast.palette.default.background(GtkState::Normal),
        adwaita.palette.default.background(GtkState::Normal)
    );
}
