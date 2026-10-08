# Packaging and development loop

Epic: caminito-v0.1
Type: implementation
Status: resolved
Milestone: M1
Blocked by: 18
Map: [Caminito v0.1](../caminito-v0.1/map.md)

## What to build

The packaging and running pieces M1 needs, so the milestone's exit is run on an installed `caminito` (16, 17).

- **Top-level `meson.build` (16):** a project with `subproject_dir: 'helpers'`. It has no subprojects and installs no data yet; M2 and M5 fill it in.
- **`packaging/arch/caminito-git/PKGBUILD` (16):**
  - `git+file://` from `$startdir` for the committed `HEAD`;
  - `pkgver()` from `git describe --long --tags`, with the `r<count>.<hash>` fallback;
  - `cargo fetch --locked`, `cargo build --frozen --release` and `cargo test --frozen`, then `meson setup`, `meson compile` and `meson install`;
  - `install -Dm755 target/release/caminito`;
  - only the dependencies the binary links now, found by `namcap`.
- **CI (16):** a job that runs `makepkg` and `namcap` on the PKGBUILD and the built package when `packaging/arch/` changes.
- **`tools/dev-nested` (14):** runs the installed `caminito` (or `--bin` for `target/`) under `dbus-run-session` with a scratch `XDG_CONFIG_HOME`, and passes through extra arguments.
- **`docs/testing/checklist.md` (14):** the format (steps, expected result, debug tool, milestone tag), and M1's items:
  - a nested run uses its private bus;
  - dconf goes to the scratch config;
  - the host session's systemd environment is untouched before and after a nested run (`systemctl --user show-environment`).

## Acceptance criteria

- [x] `makepkg -si` in `packaging/arch/caminito-git/` builds and installs `caminito`, and `namcap` reports nothing unexplained.
- [x] `tools/dev-nested` starts the installed `caminito` nested, and the host session's systemd environment is the same before and after.
- [x] The PKGBUILD job is green in CI.
- [x] Closing M1 (after [22](22-xdg-shell-and-input.md)): M1's exit from [17](17-milestone-slicing.md) is met on the installed package, and the checklist results are in a `Close M1 Nested compositor` commit.
