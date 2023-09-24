mod android;
mod apple;
mod github;
mod linux;
mod public_site;
mod secrets;
mod server;
mod utils;
mod version;
mod windows;

use cli_rs::arg::Arg;
use cli_rs::command::Command;
use cli_rs::parser::Cmd;
use version::BumpType;

use crate::utils::root;

fn main() {
    // Fail fast if we're invoking from the wrong location
    root();

    Command::name("releaser")
        .description("Lockbook's release automation")
        .subcommand(
            Command::name("bump-versions")
                .input(Arg::name("bump-type").default(BumpType::Patch))
                .handler(|bump| version::bump(bump.get())),
        )
        .subcommand(Command::name("github-release").handler(github::create_release))
        .subcommand(Command::name("all").handler(|| {
            if cfg!(target_os = "macos") {
                github::create_release()?;
                apple::release()?;
            }

            if cfg!(target_os = "linux") {
                server::deploy()?;
                linux::release()?;
                android::release()?;
                public_site::release()?;
            }

            if cfg!(target_os = "windows") {
                windows::release()?;
            }
            Ok(())
        }))
        .subcommand(Command::name("server").handler(server::deploy))
        .subcommand(Command::name("apple").handler(apple::release))
        .subcommand(Command::name("android").handler(android::release))
        .subcommand(Command::name("windows").handler(windows::release))
        .subcommand(Command::name("public-site").handler(public_site::release))
        .subcommand(Command::name("linux").handler(linux::release))
        .with_completions()
        .parse();
}
