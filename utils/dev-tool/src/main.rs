mod android;
mod apple;
mod server;
mod setup;
mod utils;
mod workspace;

use std::path::PathBuf;
use std::{env, fs};
use structopt::StructOpt;

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(about = "Lockbook's development and ci tool.")]
enum Commands {
    // CI steps in order --------------
    /// Check the formatting of the workspace
    CheckWorkspaceFmt,

    /// Check the lint of the workspace
    CheckWorkspaceClippy,

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
    server_dbs_dir: PathBuf,
    commit_hash: String,
}

fn main() {
    let root_dir = utils::root_dir();
    let dev_dir = utils::dev_dir();
    let target_dir = utils::target_dir(&dev_dir, &root_dir);
    let server_dbs_dir = utils::server_dbs_dir(&dev_dir);

    fs::create_dir_all(&dev_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();
    fs::create_dir_all(&server_dbs_dir).unwrap();

    env::set_var("CARGO_TARGET_DIR", &target_dir.to_str().unwrap());

    let tool_env = ToolEnvironment {
        root_dir,
        target_dir,
        server_dbs_dir,
        commit_hash: utils::get_commit_hash(),
    };

    use Commands::*;
    match Commands::from_args() {
        VerifyCIEnvironment => setup::verify_ci_environment(),
        CheckWorkspaceFmt => workspace::fmt_workspace(&tool_env),
        CheckWorkspaceClippy => workspace::clippy_workspace(&tool_env),
        CheckAndroidFmt => android::fmt_android(&tool_env),
        CheckAndroidLint => android::lint_android(&tool_env),
        MakeKotlinLibs => android::make_android_libs(&tool_env),
        MakeKotlinTestLib => android::make_android_test_lib(&tool_env),
        MakeSwiftTestLib => apple::make_swift_test_lib(&tool_env),
        RunServer => server::run_server_detached(&tool_env),
        RunRustTests => server::run_rust_tests(&tool_env),
        RunKotlinTests => android::run_kotlin_tests(&tool_env),
        RunSwiftTests => apple::run_swift_tests(&tool_env),
        KillServer => server::kill_server(),
    }
}
