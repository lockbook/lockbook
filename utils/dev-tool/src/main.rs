use crate::error::CliError;
use std::env;
use std::path::PathBuf;
use structopt::StructOpt;

pub mod error;
pub mod fmt;
pub mod lint;
pub mod tests;
pub mod utils;

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(about = "The best place to store and share thoughts.")]
enum Commands {
    /// Check the formatting
    FmtCheck,

    /// Check the lint
    ClippyCheck,

    /// Check the formatting of the android client
    AndroidFmtCheck,

    /// Check the lint of the android client
    AndroidLintCheck,

    /// Build server
    BuildServer,

    /// Run server detached
    LaunchServer,

    /// Run all rust tests
    RunRustTests,

    /// Run kotlin integration tests
    RunKotlinTests,

    /// Run swift integration tests
    RunSwiftTests,

    /// Kill server for commit hash
    KillServer,
}

pub struct ToolEnvironment {
    root_dir: PathBuf,
    target_dir: PathBuf,
    commit_hash: String,
}

impl ToolEnvironment {
    pub fn new() -> Result<ToolEnvironment, CliError> {
        let target_dir = utils::get_target_dir()?;

        env::set_var("CARGO_TARGET_DIR", &target_dir);

        Ok(ToolEnvironment {
            root_dir: utils::get_root_env_dir()?,
            target_dir: PathBuf::from(target_dir),
            commit_hash: utils::get_commit_hash()?,
        })
    }
}

fn main() {
    let exit_code = match parse_and_run() {
        Ok(_) => 0,
        Err(err) => {
            if let Some(msg) = err.0 {
                println!("{}", msg);
            }
            1
        }
    };

    std::process::exit(exit_code)
}

fn parse_and_run() -> Result<(), CliError> {
    let tool_env = ToolEnvironment::new()?;

    use Commands::*;
    match Commands::from_args() {
        FmtCheck => fmt::fmt_workspace(tool_env),
        ClippyCheck => lint::clippy_workspace(tool_env),
        AndroidFmtCheck => fmt::fmt_android(tool_env),
        AndroidLintCheck => lint::lint_android(tool_env),
        BuildServer => tests::build_server(tool_env),
        LaunchServer => tests::launch_server_detached(tool_env),
        RunRustTests => tests::run_rust_tests(tool_env),
        RunKotlinTests => tests::run_kotlin_tests(tool_env),
        RunSwiftTests => tests::run_swift_tests(tool_env),
        KillServer => tests::kill_server(tool_env),
    }
}
