use std::fs::{self};
use std::panic::Location;
use std::path::PathBuf;
use std::process::{Command, Output};

use cli_rs::cli_error::{CliError, CliResult};

use crate::places::root;

pub trait CommandRunner {
    fn assert_success(&mut self) -> CliResult<()>;
    fn success_output(&mut self) -> CliResult<Output>;
}

impl CommandRunner for Command {
    #[track_caller]
    fn assert_success(&mut self) -> CliResult<()> {
        if !self.status().unwrap().success() {
            Err(CliError {
                msg: format!(
                    "{self:?} did not exit successfully\ninvoked at: {}",
                    Location::caller()
                ),
                status: self.status().unwrap().code().unwrap(),
            })
        } else {
            Ok(())
        }
    }

    #[track_caller]
    fn success_output(&mut self) -> CliResult<Output> {
        let out = self.output().unwrap();

        if !out.status.success() {
            return Err(CliError {
                msg: format!(
                    "{self:?} did not exit successfully\ninvoked at: {}",
                    Location::caller()
                ),
                status: self.status().unwrap().code().unwrap(),
            });
        }

        Ok(out)
    }
}

pub fn update_self() -> CliResult<()> {
    Command::new("cargo")
        .args(["install", "--path", "utils/lbdev"])
        .current_dir(root())
        .assert_success()
}

pub fn fish_completions() -> CliResult<()> {
    let home = std::env::var("HOME").unwrap();
    let home = PathBuf::from(home);
    let completions = home.join(".config/fish/completions/lbdev.fish");

    let output = Command::new("lbdev")
        .args(["completions", "fish"])
        .output()
        .unwrap();

    fs::write(completions, output.stdout).unwrap();

    Ok(())
}
