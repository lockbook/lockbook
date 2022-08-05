#[derive(Debug)]
pub struct CliError(pub Option<String>);

impl CliError {
    pub fn print(&self) {
        if let Some(msg) = &self.0 {
            println!("{}", msg)
        }
    }

    pub fn basic_error() -> CliError {
        CliError(None)
    }
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError(Some(format!("{:?}", e)))
    }
}

impl From<dotenv::Error> for CliError {
    fn from(e: dotenv::Error) -> Self {
        CliError(Some(format!("{:?}", e)))
    }
}

impl From<serde_json::Error> for CliError {
    fn from(e: serde_json::Error) -> Self {
        CliError(Some(format!("{:?}", e)))
    }
}

impl From<std::env::VarError> for CliError {
    fn from(e: std::env::VarError) -> Self {
        CliError(Some(format!("{:?}", e)))
    }
}
