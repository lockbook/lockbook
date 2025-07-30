mod android;
mod apple;
mod server;
mod setup;
mod utils;
mod workspace;
mod ci;

use std::fs;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;

use cli_rs::command::{Command, ParserInfo};
use cli_rs::parser::Cmd;

#[derive(Debug, PartialEq, Parser)]
enum Commands {
    // CI steps in order --------------
    /// Check the formatting of the workspace
    CheckWorkspaceFmt,

    /// Check the lint of the workspace
    CheckWorkspaceClippy,

    /// Check if there are any unused deps in the workspace
    CheckWorkspaceUdeps,

    /// Run the server detached
    RunServer,

    /// Run all rust tests
    RunRustTests,

    /// Check the formatting of the android client
    CheckAndroidFmt,

    /// Check the lint of the android client
    CheckAndroidLint,

    /// Run kotlin integration tests
    RunKotlinTests,

    /// Run the swift integration tests
    RunSwiftTests,

    /// Print server logs
    PrintServerLogs,

    // Check if cargo.lock is in sync with cargo.toml
    AssertGitClean,

    /// Kill the server for commit hash
    KillServer,

    // End of CI steps --------------
    /// Verify CI environment
    VerifyCIEnvironment,

    /// Make kotlin jni libs
    MakeKotlinLibs,

    /// Make kotlin jni libs for tests
    MakeKotlinTestLib,

    /// Make swift jni libs for tests
    MakeSwiftTestLib,
}

pub struct ToolEnvironment {
    root_dir: PathBuf,
    target_dir: PathBuf,
}

fn main2() {
    let root_dir = utils::root_dir();
    let target_dir = utils::target_dir(&root_dir);

    fs::create_dir_all(&target_dir).unwrap();

    let tool_env = ToolEnvironment { root_dir, target_dir };

    use Commands::*;
    match Commands::parse() {
        VerifyCIEnvironment => setup::verify_ci_environment(),
        CheckWorkspaceFmt => workspace::fmt_workspace(&tool_env),
        CheckWorkspaceClippy => workspace::clippy_workspace(&tool_env),
        CheckWorkspaceUdeps => workspace::udeps_workspace(&tool_env),
        CheckAndroidFmt => android::fmt_android(&tool_env),
        CheckAndroidLint => android::lint_android(&tool_env),
        MakeKotlinLibs => android::make_android_libs(&tool_env),
        MakeKotlinTestLib => android::make_android_test_lib(&tool_env),
        MakeSwiftTestLib => apple::make_swift_test_lib(&tool_env),
        RunServer => server::run_server_detached(&tool_env),
        RunRustTests => server::run_rust_tests(&tool_env),
        RunKotlinTests => android::run_kotlin_tests(&tool_env),
        RunSwiftTests => apple::run_swift_tests(&tool_env),
        PrintServerLogs => server::print_server_logs(&tool_env),
        AssertGitClean => workspace::assert_git_clean(&tool_env),
        KillServer => server::kill_server(&tool_env),
    }
}

fn main() {
    Command::name("lbdev")
        .description("Tool for maintainers to dev, check and release Lockbook.")
        .with_completions()
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(
            Command::name("ci")
                .description("The commands run by CI. Sourcing dependencies is out of scope for this program")
                .subcommand(Command::name("fmt").handler(ci::clippy))
                
        )
        .parse();
}
