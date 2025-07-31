use std::panic::Location;
use std::process::Command;

use cli_rs::cli_error::{CliError, CliResult};

pub trait CommandRunner {
    fn assert_success(&mut self) -> CliResult<()>;
}

impl CommandRunner for Command {
    #[track_caller]
    fn assert_success(&mut self) -> CliResult<()> {
        if !self.status().unwrap().success() {
            Err(CliError {
                msg: format!(
                    "{self:?} did not exist successfully\ninvokded at: {}",
                    Location::caller()
                ),
                status: self.status().unwrap().code().unwrap(),
            })
        } else {
            Ok(())
        }
    }
}
