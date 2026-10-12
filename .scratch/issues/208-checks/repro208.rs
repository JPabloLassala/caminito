//! The real caja against Caminito on the headless backend, with two outputs: a right click on
//! the output that isn't the primary one must pop up the desktop's menu (208). It isn't in
//! the suite, since it needs `caja` installed. To run it, from the repository's root:
//!
//! ```sh
//! cp .scratch/issues/208-checks/repro208.rs caminito/src/tests/
//! echo 'mod repro208;' >> caminito/src/tests/mod.rs
//! scratch=$(mktemp -d) && mkdir -m 700 $scratch/rt $scratch/home
//! XDG_RUNTIME_DIR=$scratch/rt REPRO_SCRATCH=$scratch dbus-run-session -- \
//!     cargo test -p caminito --lib repro208 -- --ignored --nocapture 2>&1 | grep REPRO
//! ```
//!
//! and take both out of `caminito/src/tests/` again. `$scratch/caja.log` has caja's
//! `WAYLAND_DEBUG=1` log. `CAJA=<path>` names another `caja`, such as `src/caja` in a built
//! tree. `XDG_RUNTIME_DIR` and caja's `HOME` are scratch ones, and its settings are kept in
//! memory, so that caja can't reach the session the test is run from; the path must be short
//! enough for a socket's.

use std::time::Duration;

use smithay::output::Output;

use super::fixture::Fixture;

const BTN_RIGHT: u32 = 0x111;

fn layers(f: &Fixture) -> String {
    let dump: serde_json::Value = serde_json::from_str(&f.state.caminito.state_dump()).unwrap();
    let mut out = Vec::new();
    for output in dump["outputs"].as_array().unwrap() {
        let names: Vec<_> = output["layer_surfaces"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l["namespace"].as_str().unwrap().to_owned())
            .collect();
        out.push(format!("{}: {names:?}", output["name"].as_str().unwrap()));
    }
    out.join(" | ")
}

fn pixel(f: &mut Fixture, output: &Output, at: (i32, i32)) -> Option<u32> {
    f.state.backend.headless_mut().pixel(output, at.into())
}

fn popups(scratch: &str) -> usize {
    std::fs::read_to_string(format!("{scratch}/caja.log"))
        .unwrap()
        .lines()
        .filter(|line| line.contains("-> zwlr_layer_surface_v1") && line.contains("get_popup("))
        .count()
}

#[test]
#[ignore]
fn repro208() {
    let scratch = std::env::var("REPRO_SCRATCH").unwrap();
    let caja = std::env::var("CAJA").unwrap_or_else(|_| "/usr/bin/caja".into());
    let mut f = Fixture::new();
    let first = f.plug((600, 340), (1920, 1080));
    let second = f.plug((600, 340), (2560, 1440));
    f.start_dbus_with_names();
    f.choose_primary_output(&first);
    f.dispatch_for(Duration::from_millis(500));
    let bare = pixel(&mut f, &second, (1300, 740));

    let socket = f.state.caminito.add_socket().unwrap();
    let log = std::fs::File::create(format!("{scratch}/caja.log")).unwrap();
    let mut caja = std::process::Command::new(caja)
        .arg("--force-desktop")
        .env("WAYLAND_DISPLAY", &socket)
        .env("WAYLAND_DEBUG", "1")
        .env("HOME", format!("{scratch}/home"))
        .env("XDG_CONFIG_HOME", format!("{scratch}/home/.config"))
        .env("GSETTINGS_BACKEND", "memory")
        .env("GDK_BACKEND", "wayland")
        .env_remove("DISPLAY")
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    f.dispatch_for(Duration::from_secs(6));
    eprintln!("REPRO layers: {}", layers(&f));
    let covered = pixel(&mut f, &second, (1300, 740));
    eprintln!("REPRO the second output's pixel, bare then with caja: {bare:08x?} {covered:08x?}");

    // The middle of the second output, which is to the right of the first.
    f.pointer_move(1920.0 + 1280.0, 720.0);
    f.pointer_button(BTN_RIGHT, true);
    f.pointer_button(BTN_RIGHT, false);
    f.dispatch_for(Duration::from_secs(2));
    let popups = popups(&scratch);
    let menu = pixel(&mut f, &second, (1300, 740));
    eprintln!("REPRO popups asked of a layer surface: {popups}");
    eprintln!("REPRO the second output's pixel under the menu: {menu:08x?}");
    let verdict = if popups > 0 && menu != covered { "PASS" } else { "FAIL" };
    eprintln!("REPRO verdict: {verdict}");

    // The menus follow the primary output and the monitors. A click away from the menu
    // closes it first.
    f.click(1920.0 + 200.0, 200.0);
    let step = |f: &mut Fixture, what: &str| {
        f.dispatch_for(Duration::from_secs(3));
        eprintln!("REPRO {what}: {}", layers(f));
    };
    f.choose_primary_output(&second);
    step(&mut f, "the second chosen");
    f.unplug(&first);
    step(&mut f, "the first unplugged");
    let _first = f.replug(&first);
    step(&mut f, "the first replugged");
    f.unplug(&second);
    step(&mut f, "the second unplugged");
    let _second = f.replug(&second);
    step(&mut f, "the second replugged");
    eprintln!("REPRO caja status: {:?}", caja.try_wait());
    let _ = caja.kill();
}
