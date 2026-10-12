//! `caminito-wallpaperd` (210, ADR 0031): draws each output's wallpaper. A Session starts it,
//! and starts it again if it exits.

use gio::glib;

fn main() -> std::process::ExitCode {
    match caminito_wallpaper::drawer::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(err) => {
            glib::g_critical!("caminito-wallpaperd", "{err}");
            std::process::ExitCode::FAILURE
        }
    }
}
