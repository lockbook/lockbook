use structopt::StructOpt;
use crate::error::CliError;

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

    /// Run server detached
    LaunchServer,

    /// Run all rust tests
    RustTests,

    /// Run kotlin integration tests
    KotlinTests,

    /// Run apple integration tests
    AppleTests,
}

fn main() {
    use Commands::*;
    let result = match Commands::from_args() {
        FmtCheck => fmt::fmt_workspace(),
        ClippyCheck => lint::clippy_workspace(),
        AndroidFmtCheck => fmt::fmt_android(),
        AndroidLintCheck => lint::lint_android(),
        LaunchServer => Ok(()),
        RustTests => Ok(()),
        KotlinTests => Ok(()),
        AppleTests => Ok(()),
    };

    let exit_code = match result {
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
