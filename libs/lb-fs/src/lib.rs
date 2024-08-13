use crate::core::AsyncCore;
use crate::fs_impl::Drive;
use crate::mount::{mount, umount};
use cli_rs::cli_error::{CliError, CliResult};
use nfsserve::tcp::{NFSTcp, NFSTcpListener};
use std::io;
use std::io::IsTerminal;
use std::process::exit;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

pub mod cache;
pub mod core;
pub mod fs_impl;
pub mod logger;
pub mod mount;
pub mod utils;

impl Drive {
    /// executing this from within an async context will panic
    pub fn init() -> Self {
        let ac = AsyncCore::init();
        let data = Arc::default();

        Self { ac, data }
    }

    #[tokio::main]
    pub async fn import(&self) -> CliResult<()> {
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
    pub async fn mount(self) -> CliResult<()> {
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
                info!("will sync in 1 second");
                tokio::time::sleep(Duration::from_secs(1)).await;
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
