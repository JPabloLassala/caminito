use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::process::ExitCode;

use caminito::session::SessionCommand;

fn main() -> ExitCode {
    let args = match Args::parse(std::env::args_os().skip(1)) {
        Ok(args) => args,
        Err(err) => {
            caminito::logging::init(false);
            tracing::error!("{err}");
            return ExitCode::FAILURE;
        }
    };
    caminito::logging::init(args.session.is_some());

    let kind = caminito::backend::Kind::choose(
        std::env::var_os("WAYLAND_DISPLAY").as_deref(),
        std::env::var_os("DISPLAY").as_deref(),
    );
    match caminito::run(kind, args.session) {
        Ok(status) => ExitCode::from(status),
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
    /// What to run as the Session (08), rather than bare.
    session: Option<SessionCommand>,
}

impl Args {
    fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Self, String> {
        let mut parsed = Args::default();
        for arg in args {
            if arg == "--session" {
                parsed.session = Some(SessionCommand::mate_session());
            } else if let Some(command) = arg.as_bytes().strip_prefix(b"--session=") {
                let command = SessionCommand::parse(OsStr::from_bytes(command))
                    .ok_or("--session= needs a command")?;
                parsed.session = Some(command);
            } else {
                return Err(format!("unknown argument {}", arg.to_string_lossy()));
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

    /// The program and arguments `args` make the Session's command.
    fn session(args: &[&str]) -> Option<Vec<String>> {
        let command = parse(args).unwrap().session?;
        let words = std::iter::once(command.program()).chain(command.args());
        Some(
            words
                .map(|word| word.to_str().unwrap().to_owned())
                .collect(),
        )
    }

    #[test]
    fn no_arguments_is_a_bare_run() {
        assert_eq!(parse(&[]), Ok(Args { session: None }));
    }

    #[test]
    fn session_runs_mate_session_without_its_acceleration_check() {
        assert_eq!(
            session(&["--session"]).unwrap(),
            ["mate-session", "--disable-acceleration-check"]
        );
    }

    #[test]
    fn session_with_a_command_runs_it_instead() {
        assert_eq!(session(&["--session=foot"]).unwrap(), ["foot"]);
    }

    #[test]
    fn a_session_commands_words_are_its_arguments() {
        assert_eq!(
            session(&["--session=foot  --title scratch"]).unwrap(),
            ["foot", "--title", "scratch"]
        );
    }

    #[test]
    fn session_with_an_empty_command_is_refused() {
        assert!(parse(&["--session="]).is_err());
        assert!(parse(&["--session= "]).is_err());
    }

    #[test]
    fn unknown_arguments_are_refused() {
        assert!(parse(&["--nested"]).is_err());
        assert!(parse(&["--sessions"]).is_err());
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
