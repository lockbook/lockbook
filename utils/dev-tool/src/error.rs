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
