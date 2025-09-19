mod ci;
mod local;
mod places;
mod releaser;
mod utils;

use cli_rs::arg::Arg;
use cli_rs::command::Command;
use cli_rs::parser::Cmd;
use releaser::version::BumpType;

fn main() {
    Command::name("lbdev")
        .description("Tool for maintainers to dev, check and release Lockbook.")
        .with_completions()
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(
            Command::name("apple")
                .description("utilties for building for apple development or launching lockbook on apple devices.")
                .subcommand(
                    Command::name("ws")
                        .description("build workspace for a given apple platform")
                        .subcommand(Command::name("all").handler(local::apple_ws_all))
                        .subcommand(Command::name("macos").handler(local::apple_ws_macos))
                        .subcommand(Command::name("ios").handler(local::apple_ws_ios))
                )
                .subcommand(
                    Command::name("run")
                        .description("build workspace and run on a given macOS or iOS device")
                        .subcommand(Command::name("macos").description("runs lockbook macOS on this device").handler(local::apple_run_macos))
                        .subcommand(Command::name("ios").description("deploys lockbook to a connected iOS device").input(Arg::str("device-name").description("looks up a device udid by the given friendly name, builds lb and deploys it on that device.").completor(local::apple_device_name_completor)).handler(|device| local::apple_run_ios(device.get())))
                )
        )
        .subcommand(
            Command::name("android")
                .description("utilties for building for android development")
                .subcommand(
                    Command::name("ws")
                        .description("build workspace for android").handler(local::android_ws)
                )
        )
        .subcommand(Command::name("server").handler(local::server))
        .subcommand(
             Command::name("releaser")
                .description("Lockbook's release automation")
                .subcommand(
                    Command::name("bump-versions")
                        .input(Arg::name("bump-type").default(BumpType::Today))
                        .handler(|bump| releaser::version::bump(bump.get())),
                    )
                .subcommand(Command::name("github-release").handler(releaser::github::create_release))
                .subcommand(Command::name("server").handler(releaser::server::deploy))
                .subcommand(Command::name("apple")
                    .subcommand(Command::name("all").handler(releaser::apple::release))
                    .subcommand(Command::name("cli").handler(releaser::apple::cli::release))
                    .subcommand(Command::name("ios").handler(|| releaser::apple::ios::release(true)))
                    .subcommand(Command::name("mac-app-store").handler(|| releaser::apple::mac::release(true, false, true)))
                    .subcommand(Command::name("mac-gh").handler(|| releaser::apple::mac::release(true, true, false)))
                )
                .subcommand(Command::name("android")
                    .subcommand(Command::name("all").handler(|| releaser::android::release(true, true)))
                    .subcommand(Command::name("play-store").handler(|| releaser::android::release(true, false)))
                    .subcommand(Command::name("gh").handler(|| releaser::android::release(false, true)))
                )
                .subcommand(
                    Command::name("windows")
                        .subcommand(Command::name("all").handler(releaser::windows::release))
                        .subcommand(Command::name("cli").handler(releaser::windows::cli::release))
                        .subcommand(Command::name("desktop").handler(releaser::windows::desktop::release)),
                )
                .subcommand(
                    Command::name("linux")
                        .subcommand(Command::name("all").handler(releaser::linux::release))
                        .subcommand(
                            Command::name("cli")
                                .subcommand(Command::name("all").handler(releaser::linux::cli::release))
                                .subcommand(Command::name("gh").handler(releaser::linux::cli::bin_gh))
                                .subcommand(Command::name("deb").handler(releaser::linux::cli::upload_deb))
                                .subcommand(Command::name("snap").handler(releaser::linux::cli::update_snap))
                                .subcommand(Command::name("aur").handler(releaser::linux::cli::update_aur)),
                        )
                        .subcommand(
                            Command::name("desktop")
                                .subcommand(Command::name("all").handler(releaser::linux::desktop::release))
                                .subcommand(Command::name("gh").handler(releaser::linux::desktop::upload_gh))
                                .subcommand(Command::name("deb").handler(releaser::linux::desktop::upload_deb_gh))
                                .subcommand(Command::name("snap").handler(releaser::linux::desktop::update_snap))
                                .subcommand(Command::name("aur").handler(releaser::linux::desktop::update_aur)),
                        )
                )
                .subcommand(
                    Command::name("publish-crate")
                        .input(Arg::name("package"))
                        .handler(|package| releaser::crates_io::release_crate(package.get())),
                )
        )
        .subcommand(
            Command::name("update")
                .description("update the lbdev binary to the latest from your current source tree.")
                .handler(utils::update_self)
        )
        .subcommand(
            Command::name("fish-completions")
                .description("install completions for the fish shell")
                .handler(utils::fish_completions)

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
                .subcommand(Command::name("android-lint").handler(ci::lint_android))
                .subcommand(Command::name("assert-git-clean").handler(ci::assert_git_clean))
                .subcommand(Command::name("assert-no-udeps").handler(ci::assert_no_udeps))
        )
        .parse();
}
