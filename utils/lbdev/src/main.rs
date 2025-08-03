mod ci;
mod local;
mod places;
mod utils;

use cli_rs::arg::Arg;
use cli_rs::command::Command;
use cli_rs::parser::Cmd;

fn main() {
    Command::name("lbdev")
        .description("Tool for maintainers to dev, check and release Lockbook.")
        .with_completions()
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(
            Command::name("update")
                .description("update the lbdev binary to the latest from your current source tree.")
                .handler(utils::update_self)
        )
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
                .subcommand(Command::name("assert-no-udeps").handler(ci::assert_no_udeps))
        )
        .subcommand(
            Command::name("apple")
                .description("utilties for building for apple development or launching lockbook on apple devices.")
                .subcommand(
                    Command::name("ws")
                        .subcommand(Command::name("all").handler(local::apple_ws_all))
                        .subcommand(Command::name("macos").handler(local::apple_ws_macos))
                        .subcommand(Command::name("ios").handler(local::apple_ws_ios))
                )
                .subcommand(
                    Command::name("run")
                        .subcommand(Command::name("macos").handler(local::apple_run_macos))
                        .subcommand(Command::name("ios").input(Arg::str("device-name").completor(local::apple_device_name_completor)).handler(|device| local::apple_run_ios(device.get())))
                )
        )
        .parse();
}
