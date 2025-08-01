use std::panic::Location;
use std::process::Command;

use cli_rs::cli_error::{CliError, CliResult};

use crate::places::root;

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

pub fn update_self() -> CliResult<()> {
    Command::new("cargo")
        .args(["install", "--path", "utils/lbdev"])
        .current_dir(root())
        .assert_success()
}
