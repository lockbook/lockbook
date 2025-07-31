mod ci;
mod places;
mod utils;
mod workspace;

use cli_rs::command::Command;
use cli_rs::parser::Cmd;

fn main() {
    Command::name("lbdev")
        .description("Tool for maintainers to dev, check and release Lockbook.")
        .with_completions()
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(
            Command::name("ci")
                .description("The commands run by CI. Sourcing dependencies is out of scope for this program")
                .subcommand(Command::name("fmt").handler(ci::fmt))
                .subcommand(Command::name("clippy").handler(ci::clippy))
                .subcommand(Command::name("start-server").handler(ci::run_server_detached))
                .subcommand(Command::name("rust-tests").handler(ci::run_rust_tests))
                .subcommand(Command::name("kill-server").handler(ci::kill_server))
                .subcommand(Command::name("server-logs").handler(ci::print_server_logs))
                .subcommand(Command::name("android-fmt").handler(ci::fmt_android))
                .subcommand(Command::name("server-logs").handler(ci::lint_android))
                .subcommand(Command::name("assert-git-clean").handler(ci::assert_git_clean))
        )
        .parse();
}
