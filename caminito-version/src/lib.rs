//! The version string of this build of Caminito (182): the Cargo version, and then the
//! build's revision in brackets, `0.1.0 (r672.989d558)`.
//!
//! `caminito --version` prints it and caminito-about shows it. It's a crate so that one
//! build script finds the revision for both, and they can't disagree.

#![forbid(unsafe_code)]

// Only its tests are this crate's: the build script is what calls it.
#[cfg(test)]
mod string;

/// The version string, as the build script made it.
pub const VERSION: &str = env!("CAMINITO_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_string_starts_with_the_cargo_version() {
        let cargo = env!("CARGO_PKG_VERSION");
        assert!(
            VERSION == cargo || VERSION.starts_with(&format!("{cargo} (")),
            "{VERSION}"
        );
    }
}
