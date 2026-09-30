use std::ffi::OsString;
use std::process::ExitCode;

fn main() -> ExitCode {
    camino::logging::init();

    let args = match Args::parse(std::env::args_os().skip(1)) {
        Ok(args) => args,
        Err(err) => {
            tracing::error!("{err}");
            return ExitCode::FAILURE;
        }
    };
    if args.session {
        tracing::error!("--session isn't supported yet");
        return ExitCode::FAILURE;
    }

    let kind = camino::backend::Kind::choose(
        std::env::var_os("WAYLAND_DISPLAY").as_deref(),
        std::env::var_os("DISPLAY").as_deref(),
    );
    match camino::run(kind) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("{}", report(err.as_ref()));
            ExitCode::FAILURE
        }
    }
}

/// An error and its chain of causes, which Smithay's errors leave out of their messages.
fn report(err: &dyn std::error::Error) -> String {
    let mut report = err.to_string();
    let mut source = err.source();
    while let Some(cause) = source {
        report.push_str(": ");
        report.push_str(&cause.to_string());
        source = cause.source();
    }
    report
}

#[derive(Debug, Default, PartialEq)]
struct Args {
    /// Run as the Session (08), rather than bare.
    session: bool,
}

impl Args {
    fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut parsed = Args::default();
        for arg in args {
            match arg.to_str() {
                Some("--session") => parsed.session = true,
                _ => return Err(format!("unknown argument {}", arg.to_string_lossy())),
            }
        }
        Ok(parsed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Args, String> {
        Args::parse(args.iter().map(OsString::from))
    }

    #[test]
    fn no_arguments_is_a_bare_run() {
        assert_eq!(parse(&[]), Ok(Args { session: false }));
    }

    #[test]
    fn session_is_parsed() {
        assert_eq!(parse(&["--session"]), Ok(Args { session: true }));
    }

    #[test]
    fn unknown_arguments_are_refused() {
        assert!(parse(&["--nested"]).is_err());
    }

    #[derive(Debug)]
    struct Wrapped(std::io::Error);

    impl std::fmt::Display for Wrapped {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("failed to initialize an event loop")
        }
    }

    impl std::error::Error for Wrapped {
        fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
            Some(&self.0)
        }
    }

    #[test]
    fn errors_are_reported_with_their_causes() {
        let cause = std::io::Error::other("no Wayland or X11 display");
        let err = Wrapped(cause);
        assert_eq!(
            report(&err),
            "failed to initialize an event loop: no Wayland or X11 display"
        );
    }
}
