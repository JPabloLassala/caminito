//! The real msd against Camino on the headless backend through two hot-plugs (88). It isn't
//! in the suite, since it needs `mate-settings-daemon` installed. To run it, from the
//! repository's root:
//!
//! ```sh
//! cp .scratch/camino-v0.1/issues/88-hot-plug-check/repro88.rs camino/src/tests/
//! echo 'mod repro88;' >> camino/src/tests/mod.rs
//! scratch=$(mktemp -d) && mkdir -m 700 $scratch/rt $scratch/config
//! cp .scratch/camino-v0.1/issues/88-hot-plug-check/msd.sh $scratch/
//! XDG_RUNTIME_DIR=$scratch/rt REPRO_SCRATCH=$scratch \
//!     cargo test -p camino --lib repro88 -- --ignored --nocapture 2>&1 | grep REPRO
//! ```
//!
//! and take both out of `camino/src/tests/` again. `$scratch/msd.log` has msd's
//! `WAYLAND_DEBUG=1` log. `MSD=<path>` names another `mate-settings-daemon`, which loads the
//! plugins of its own prefix. `XDG_RUNTIME_DIR` is a scratch one so that msd, on its own bus
//! and dconf database, can't reach the session the test is run from; its path must be short
//! enough for a socket's.

use std::time::Duration;

use super::fixture::Fixture;

fn counts(f: &Fixture) -> String {
    let dump: serde_json::Value = serde_json::from_str(&f.state.camino.state_dump()).unwrap();
    let mut out = Vec::new();
    for output in dump["outputs"].as_array().unwrap() {
        let layers = output["layer_surfaces"].as_array().unwrap();
        let names: Vec<_> = layers
            .iter()
            .map(|l| l["namespace"].as_str().unwrap().to_owned())
            .collect();
        out.push(format!(
            "{}: {} {:?}",
            output["name"].as_str().unwrap(),
            layers.len(),
            names
        ));
    }
    out.join(" | ")
}

#[test]
#[ignore]
fn repro88() {
    let scratch = std::env::var("REPRO_SCRATCH").unwrap();
    let mut f = Fixture::new();
    let first = f.plug((600, 340), (1920, 1080));
    let second = f.plug((600, 340), (2560, 1440));
    let socket = f.state.camino.add_socket().unwrap();
    let log = std::fs::File::create(format!("{scratch}/msd.log")).unwrap();
    let mut msd = std::process::Command::new("dbus-run-session")
        .arg("--")
        .arg(format!("{scratch}/msd.sh"))
        .env("WAYLAND_DISPLAY", &socket)
        .env("XDG_CONFIG_HOME", format!("{scratch}/config"))
        .env("MATE_WM_NAME", "Camino")
        .env("GDK_BACKEND", "wayland")
        .env_remove("DISPLAY")
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .spawn()
        .unwrap();
    let step = |f: &mut Fixture, what: &str| {
        f.dispatch_for(Duration::from_secs(4));
        eprintln!("REPRO {what}: {}", counts(f));
    };
    step(&mut f, "start");
    f.unplug(&second);
    step(&mut f, "second unplugged");
    let second = f.replug(&second);
    step(&mut f, "second replugged");
    f.unplug(&first);
    step(&mut f, "first unplugged");
    let _first = f.replug(&first);
    step(&mut f, "first replugged");
    let _ = second;
    eprintln!("REPRO msd status: {:?}", msd.try_wait());
    let _ = msd.kill();
}
