//! `caminito --version` (182), run as the user runs it: the binary itself, with nothing of
//! a desktop around it.

use std::process::Command;

#[test]
fn version_prints_the_version_string_and_starts_no_compositor() {
    let output = Command::new(env!("CARGO_BIN_EXE_caminito"))
        .arg("--version")
        // With nothing to nest in and no seat a compositor couldn't start, so status 0 says
        // none was tried.
        .env_clear()
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        format!("{}\n", caminito_version::VERSION)
    );
    assert!(output.stderr.is_empty(), "{output:?}");
}
