use cli_rs::cli_error::CliResult;
use cli_rs::command::Command;
use cli_rs::parser::Cmd;
use lb_fs::fs_impl::Drive;
use lb_fs::logger;

fn main() {
    logger::init();
    Command::name("lb-fs")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand(
            Command::name("import")
                .description("sign in and sync a lockbook account")
                .handler(import),
        )
        .subcommand(
            Command::name("mount")
                .description("start an NFS server and mount it to /tmp/lockbook")
                .handler(mount),
        )
        .parse();
}

#[tokio::main]
async fn import() -> CliResult<()> {
    Drive::import().await?;
    Ok(())
}

#[tokio::main]
async fn mount() -> CliResult<()> {
    Drive::mount().await?;
    Ok(())
}
