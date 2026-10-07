# Workspace and CI

Type: implementation
Status: resolved
Milestone: M1
Blocked by:
Map: [Caminito v0.1](../map.md)

## What to build

The empty shape of the repo, with CI running on it from the first commit.

- **Cargo workspace (15, ADR 0010):** a virtual workspace with four member crates, each compiling with a placeholder:
  - `caminito`, a library plus a `main.rs`;
  - `caminito-policy`;
  - `caminito-theme`;
  - `caminito-test-client`, with `publish = false`.
- **Smithay** is pinned by an explicit `rev` in `[workspace.dependencies]`, to the latest master commit on the day. The reason goes in the commit message (14). `caminito-policy` takes it with `default-features = false`, for geometry types only.
- **Toolchain (14):**
  - `rust-toolchain.toml` with `channel = "stable"`, rustfmt and clippy;
  - rustfmt on its defaults, and clippy with `-D warnings`;
  - `deny.toml` for cargo-deny. It allows GPL-2.0-or-later-compatible licences plus Apache-2.0, and rejects GPL-3-only crates and unknown licences.
- **Licence:** `COPYING` with GPL-2.0-or-later, and `license = "GPL-2.0-or-later"` in every crate.
- **CI (14):** `.github/workflows/ci.yml` runs in an `archlinux:latest` container on push and pull request. It runs rustfmt `--check`, clippy, cargo-deny, `cargo build`, and `cargo test --workspace`. The container installs only what the build needs so far, and later tickets add packages as they need them.
- **Weekly job (14):** `.github/workflows/weekly.yml` has two jobs:
  - a Smithay canary that builds and tests against Smithay master, and opens or updates an issue when it fails;
  - a proptest job with a large `PROPTEST_CASES`, which has nothing to run until [19](19-policy-crate.md).
- `.gitignore` for `target/`.

## Acceptance criteria

- [x] `cargo build --workspace`, `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `cargo fmt --check` and `cargo deny check` all pass locally.
- [x] `caminito-policy`'s dependency tree has no `wayland-server`, `wayland-client`, x11rb, GLib or zbus (`cargo tree -p caminito-policy`).
- [x] The push workflow is green on GitHub.
- [x] The weekly workflow has been started by hand once with `workflow_dispatch`, and both jobs run.
