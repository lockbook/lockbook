pub mod cache;
pub mod core;
pub mod fs_impl;
pub mod logger;
pub mod mount;
pub mod utils;

use cli_rs::{
    cli_error::{CliError, CliResult},
    command::Command,
    parser::Cmd,
};
use core::AsyncCore;
use fs_impl::Drive;
use mount::{mount, umount};
use nfsserve::tcp::{NFSTcp, NFSTcpListener};
use std::{
    io::{self, IsTerminal},
    process::exit,
    sync::Arc,
    time::Duration,
};
use tracing::info;

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

impl Drive {
    /// executing this from within an async context will panic
    fn init() -> Self {
        let ac = AsyncCore::init();
        let data = Arc::default();

        Self { ac, data }
    }

    #[tokio::main]
    async fn import(&self) -> CliResult<()> {
        if io::stdin().is_terminal() {
            return Err(CliError::from("to import an existing lockbook account, pipe your account string into this command, e.g.:\npbpaste | lb-fs import".to_string()));
        }

        let mut account_string = String::new();
        io::stdin()
            .read_line(&mut account_string)
            .expect("failed to read from stdin");
        account_string.retain(|c| !c.is_whitespace());

        println!("importing account...");
        self.ac.import_account(&account_string).await;

        self.ac.sync().await;

        Ok(())
    }

    #[tokio::main]
    async fn mount(self) -> CliResult<()> {
        self.prepare_caches().await;
        info!("registering sig handler");

        // capture ctrl_c and try to cleanup
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            umount().await;
            info!("cleaned up, goodbye!");
            exit(0);
        });

        // sync periodically in the background
        let syncer = self.clone();
        tokio::spawn(async move {
            loop {
                info!("will sync in 5 minutes");
                tokio::time::sleep(Duration::from_secs(300)).await;
                info!("syncing");
                syncer.sync().await;
            }
        });

        // todo have a better port selection strategy
        info!("creating server");
        let listener = NFSTcpListener::bind("127.0.0.1:11111", self).await.unwrap();

        info!("mounting");
        mount();

        info!("ready");
        listener.handle_forever().await.unwrap();
        Ok(())
    }
}
