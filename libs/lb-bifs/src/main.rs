use cli_rs::cli_error::{CliResult, Exit};
use cli_rs::command::Command;
use cli_rs::parser::Cmd;
use lb_bifs::BiFS;

fn main() {
    Command::name("lb-bifs")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(
            Command::name("push")
                .description("push local changes to lockbook")
                .handler(push),
        )
        .subcommand(
            Command::name("pull")
                .description("pull remote changes from lockbook")
                .handler(pull),
        )
        .parse()
        .exit()
}

#[tokio::main]
async fn push() -> CliResult<()> {
    let mut bifs = BiFS::init().await;
    bifs.push().await;
    Ok(())
}

#[tokio::main]
async fn pull() -> CliResult<()> {
    let mut bifs = BiFS::init().await;
    bifs.pull().await;
    Ok(())
}
