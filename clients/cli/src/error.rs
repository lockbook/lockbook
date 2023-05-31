use std::fmt;
use std::io;

pub struct CliError(pub String);

impl CliError {
    pub fn new(msg: impl ToString) -> Self {
        Self(msg.to_string())
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error: {}", self.0)
    }
}

impl From<lb::LbError> for CliError {
    fn from(err: lb::LbError) -> Self {
        Self(format!("{:?}", err))
    }
}

impl From<lb::UnexpectedError> for CliError {
    fn from(err: lb::UnexpectedError) -> Self {
        Self(format!("unexpected: {:?}", err))
    }
}

impl From<io::Error> for CliError {
    fn from(err: io::Error) -> Self {
        Self(format!("{:?}", err))
    }
}

impl From<shellwords::MismatchedQuotes> for CliError {
    fn from(value: shellwords::MismatchedQuotes) -> Self {
        CliError(format!(
            "shell input couldn't be parse as it doesn't follow UNIX standards{}",
            value
        ))
    }
}

impl From<clap::Error> for CliError {
    fn from(value: clap::Error) -> Self {
        CliError(value.to_string())
    }
}
