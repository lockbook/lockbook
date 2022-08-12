use std::path::PathBuf;
use std::{env, fs};
use structopt::StructOpt;

pub mod android;
pub mod apple;
pub mod server;
pub mod setup;
pub mod utils;
pub mod workspace;

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

    /// Kill all servers running
    KillAllServers,
}

#[derive(Clone)]
pub struct ToolEnvironment {
    root_dir: PathBuf,
    target_dir: PathBuf,
    hash_info_dir: PathBuf,
    commit_hash: String,
}

impl Default for ToolEnvironment {
    fn default() -> Self {
        let root_dir = utils::root_dir();
        let dev_dir = utils::dev_dir();
        let target_dir = utils::target_dir(&dev_dir, &root_dir);
        let hash_info_dir = utils::hash_infos_dir(&dev_dir);

        fs::create_dir_all(&dev_dir).unwrap();
        fs::create_dir_all(&hash_info_dir).unwrap();
        fs::create_dir_all(&target_dir).unwrap();

        env::set_var("CARGO_TARGET_DIR", &target_dir.to_str().unwrap());

        ToolEnvironment {
            root_dir,
            target_dir,
            hash_info_dir,
            commit_hash: utils::get_commit_hash(),
        }
    }
}

fn main() {
    let tool_env = ToolEnvironment::default();

    use Commands::*;
    match Commands::from_args() {
        VerifyCIEnvironment => setup::verify_ci_environment(),
        CheckWorkspaceFmt => workspace::fmt_workspace(tool_env),
        CheckWorkspaceClippy => workspace::clippy_workspace(tool_env),
        CheckAndroidFmt => android::fmt_android(tool_env),
        CheckAndroidLint => android::lint_android(tool_env),
        MakeKotlinLibs => android::make_android_libs(tool_env),
        MakeKotlinTestLib => android::make_android_test_lib(tool_env),
        MakeSwiftTestLib => apple::make_swift_test_lib(tool_env),
        RunServer => server::run_server_detached(tool_env),
        RunRustTests => server::run_rust_tests(tool_env),
        RunKotlinTests => android::run_kotlin_tests(tool_env),
        RunSwiftTests => apple::run_swift_tests(tool_env),
        KillServer => server::kill_server(tool_env),
        KillAllServers => server::kill_all_servers(tool_env),
    }
}
