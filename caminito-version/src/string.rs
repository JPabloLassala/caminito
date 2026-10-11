//! How the version string is put together (182). The build script compiles this file too,
//! so the string it bakes in is the one tested here.

/// The revision of a build: the packager's, from the environment, before the checkout's,
/// which `git` is only asked for when the environment names none. An empty variable names
/// none.
pub(crate) fn revision(
    environment: Option<String>,
    git: impl FnOnce() -> Option<String>,
) -> Option<String> {
    environment
        .map(|revision| revision.trim().to_owned())
        .filter(|revision| !revision.is_empty())
        .or_else(git)
}

/// A revision in the PKGBUILD's `pkgver` form, from `git rev-list --count HEAD` and
/// `git rev-parse --short=7 HEAD`.
pub(crate) fn pkgver(count: &str, hash: &str) -> String {
    format!("r{}.{}", count.trim(), hash.trim())
}

/// The version string: the Cargo version, and then the revision in brackets if the build
/// has one.
pub(crate) fn version(cargo: &str, revision: Option<&str>) -> String {
    match revision {
        Some(revision) => format!("{cargo} ({revision})"),
        None => cargo.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_revision_follows_the_cargo_version_in_brackets() {
        assert_eq!(
            version("0.1.0", Some("r672.989d558")),
            "0.1.0 (r672.989d558)"
        );
    }

    #[test]
    fn a_build_with_no_revision_has_the_cargo_version_alone() {
        assert_eq!(version("0.1.0", None), "0.1.0");
    }

    #[test]
    fn the_packagers_revision_comes_before_the_checkouts() {
        let revision = revision(Some("r672.989d558".to_owned()), || {
            panic!("git isn't asked when the environment names the revision")
        });
        assert_eq!(revision.as_deref(), Some("r672.989d558"));
    }

    #[test]
    fn the_checkouts_revision_is_used_when_the_environment_names_none() {
        let git = || Some("r1.abcdef0".to_owned());
        assert_eq!(revision(None, git).as_deref(), Some("r1.abcdef0"));
        assert_eq!(
            revision(Some(String::new()), git).as_deref(),
            Some("r1.abcdef0")
        );
    }

    #[test]
    fn a_build_outside_a_checkout_has_no_revision() {
        assert_eq!(revision(None, || None), None);
    }

    #[test]
    fn a_checkouts_revision_has_the_pkgbuilds_form() {
        assert_eq!(pkgver("672\n", "989d558\n"), "r672.989d558");
    }
}
