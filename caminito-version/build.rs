//! Finds the revision of this build and bakes the version string into the crate (182).
//!
//! The revision is `CAMINITO_REVISION`'s, which the PKGBUILD sets from its `pkgver`, or
//! else the checkout's, from `git`. A build with neither, from a tarball say, has the Cargo
//! version alone and doesn't fail.
//!
//! The checkout's revision is always `r<count>.<hash>`, which is the PKGBUILD's `pkgver` while
//! the repository has no tag. With one, `pkgver` is `git describe`'s, and only a packaged
//! build has it.

use std::path::{Path, PathBuf};
use std::process::Command;

#[path = "src/string.rs"]
mod string;

/// The variable a packager names the revision in.
const REVISION_VARIABLE: &str = "CAMINITO_REVISION";

fn main() {
    println!("cargo::rerun-if-env-changed={REVISION_VARIABLE}");
    let manifest_dir =
        PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").expect("cargo sets it"));
    watch_head(&manifest_dir);

    let revision = string::revision(std::env::var(REVISION_VARIABLE).ok(), || {
        // Caminito's own checkout, and not a repository that an unpacked tarball of it
        // happens to be below, whose revision git would give as readily.
        git(&manifest_dir, &["ls-files", "--error-unmatch", "build.rs"])?;
        let count = git(&manifest_dir, &["rev-list", "--count", "HEAD"])?;
        let hash = git(&manifest_dir, &["rev-parse", "--short=7", "HEAD"])?;
        Some(string::pkgver(&count, &hash))
    });
    let cargo = std::env::var("CARGO_PKG_VERSION").expect("cargo sets it");
    let version = string::version(&cargo, revision.as_deref());
    println!("cargo::rustc-env=CAMINITO_VERSION={version}");
}

/// Has cargo run this script again when the checkout's HEAD moves, so a commit changes the
/// string without a `cargo clean`. HEAD itself changes on a checkout of another branch, and
/// its reflog on every commit, reset and pull. Outside a checkout there is nothing to
/// watch, and the variable above is all that reruns the script.
fn watch_head(manifest_dir: &Path) {
    for file in ["HEAD", "logs/HEAD"] {
        // In a worktree these are below the main checkout's .git, which is why git is asked.
        let Some(path) = git(manifest_dir, &["rev-parse", "--git-path", file]) else {
            return;
        };
        let path = manifest_dir.join(path.trim());
        // A file that isn't there would rerun the script on every build.
        if path.exists() {
            println!("cargo::rerun-if-changed={}", path.display());
        }
    }
}

/// What a git command prints, in the checkout this crate is in. `None` if there's no git,
/// no checkout, or no commit yet.
fn git(manifest_dir: &Path, arguments: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(manifest_dir)
        .args(arguments)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}
