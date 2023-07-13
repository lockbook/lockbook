use std::fmt;
use std::io;

pub enum CliError {
    Console(String),
    Silent(String),
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CliError::Console(msg) => writeln!(f, "error: {}", msg),
            #[cfg(debug_assertions)]
            CliError::Silent(msg) => writeln!(f, "error: {}", msg),
            #[cfg(not(debug_assertions))]
            CliError::Silent(_msg) => write!(f, ""),
        }
    }
}

impl From<lb::LbError> for CliError {
    fn from(err: lb::LbError) -> Self {
        Self::Console(format!("{:?}", err))
    }
}

impl From<lb::UnexpectedError> for CliError {
    fn from(err: lb::UnexpectedError) -> Self {
        Self::Console(format!("unexpected: {:?}", err))
    }
}

impl From<io::Error> for CliError {
    fn from(err: io::Error) -> Self {
        Self::Console(format!("{:?}", err))
    }
}

impl From<shellwords::MismatchedQuotes> for CliError {
    fn from(value: shellwords::MismatchedQuotes) -> Self {
        Self::Silent(format!(
            "shell input couldn't be parsed as it doesn't follow UNIX standards{}",
            value
        ))
    }
}

impl From<clap::Error> for CliError {
    fn from(value: clap::Error) -> Self {
        Self::Silent(value.to_string())
    }
}
