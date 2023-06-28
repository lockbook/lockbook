use std::fmt;
use std::io;

pub enum CliError {
    ConsoleError(String),
    SilentError(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::ConsoleError(msg) => write!(f, "error: {}", msg),
            CliError::SilentError(_msg) => write!(f, ""),
        }
    }
}

impl From<lb::LbError> for CliError {
    fn from(err: lb::LbError) -> Self {
        Self::ConsoleError(format!("{:?}", err))
    }
}

impl From<lb::UnexpectedError> for CliError {
    fn from(err: lb::UnexpectedError) -> Self {
        Self::ConsoleError(format!("unexpected: {:?}", err))
    }
}

impl From<io::Error> for CliError {
    fn from(err: io::Error) -> Self {
        Self::ConsoleError(format!("{:?}", err))
    }
}

impl From<shellwords::MismatchedQuotes> for CliError {
    fn from(value: shellwords::MismatchedQuotes) -> Self {
        Self::SilentError(format!(
            "shell input couldn't be parse as it doesn't follow UNIX standards{}",
            value
        ))
    }
}

impl From<clap::Error> for CliError {
    fn from(value: clap::Error) -> Self {
        Self::SilentError(value.to_string())
    }
}
