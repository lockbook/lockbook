use std::fs::File;
use std::path::PathBuf;
use std::process::Command;
use std::{panic::Location, process::Stdio};

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
                    "{self:?} did not exit successfully\ninvoked at: {}",
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

pub fn fish_completions() -> CliResult<()> {
    let home = std::env::var("HOME").unwrap();
    let home = PathBuf::from(home);
    let completions_dir = home.join(".config/fish/completions");

    Command::new("lbdev")
        .args(&["completions", "fish"])
        .current_dir(completions_dir)
        .stdout(Stdio::from(File::create("lbdev.fish").unwrap()))
        .assert_success()
}
