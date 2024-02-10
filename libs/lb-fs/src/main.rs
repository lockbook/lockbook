use cli_rs::{command::Command, parser::Cmd};
use lb_fs::fs_impl::Drive;
use lb_fs::logger;

fn main() {
    logger::init();
    Command::name("lb-fs")
        .subcommand(
            Command::name("import")
                .description("sign in and sync a lockbook account")
                .handler(|| Drive::init().import()),
        )
        .subcommand(
            Command::name("mount")
                .description("start an NFS server and mount it to /tmp/lockbook")
                .handler(|| Drive::init().mount()),
        )
        .parse();
}
