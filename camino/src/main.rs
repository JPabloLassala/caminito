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

    match camino::run_nested() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!("{err}");
            ExitCode::FAILURE
        }
    }
}

#[derive(Debug, Default, PartialEq)]
struct Args {
    /// Run as the Session (08), rather than nested.
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
    fn no_arguments_is_a_nested_run() {
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
}
