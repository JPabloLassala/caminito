use std::fmt;
use std::path::PathBuf;

/// Why a theme didn't load.
#[derive(Debug, Clone, PartialEq)]
pub enum ThemeError {
    /// No theme directory has a file of this theme that Marco would load.
    NotFound { name: String },
    /// The file's root element asks for a newer format than Marco's 3.5. Marco then tries
    /// the theme's older files, so the loader does too.
    UnsupportedVersion {
        path: PathBuf,
        line: usize,
        message: String,
    },
    /// The file is one Marco would refuse.
    Invalid {
        path: PathBuf,
        line: usize,
        message: String,
    },
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThemeError::NotFound { name } => {
                write!(f, "Failed to find a valid file for theme {name}")
            }
            ThemeError::UnsupportedVersion {
                path,
                line,
                message,
            }
            | ThemeError::Invalid {
                path,
                line,
                message,
            } => write!(f, "{}:{line}: {message}", path.display()),
        }
    }
}

impl std::error::Error for ThemeError {}
