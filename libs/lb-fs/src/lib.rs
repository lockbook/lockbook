use crate::fs_impl::Drive;
use crate::mount::{mount, umount};
use cli_rs::cli_error::{CliError, CliResult};
use lb_rs::model::core_config::Config;
use lb_rs::service::sync::SyncProgress;
use lb_rs::{Lb, Uuid};
use nfs3_server::tcp::{NFSTcp, NFSTcpListener};
use std::io;
use std::io::IsTerminal;
use std::process::exit;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

pub mod cache;
pub(crate) mod file_handle;
pub mod fs_impl;
pub mod logger;
pub mod mount;
pub mod utils;

impl Drive {
    pub async fn init() -> Self {
        let lb = Lb::init(Config {
            writeable_path: Config::writeable_path("drive"),
            background_work: false,
            logs: false,
            stdout_logs: false,
            colored_logs: false,
        })
        .await
        .unwrap();

        let root = lb.root().await.map(|file| file.id).unwrap_or(Uuid::nil());

        let data = Arc::default();

        Self { lb, root, data }
    }

    pub async fn import() -> CliResult<()> {
        let drive = Self::init().await;

        if io::stdin().is_terminal() {
            return Err(CliError::from("to import an existing lockbook account, pipe your account string into this command, e.g.:\npbpaste | lb-fs import".to_string()));
        }

        let mut account_string = String::new();
        io::stdin()
            .read_line(&mut account_string)
            .expect("failed to read from stdin");
        account_string.retain(|c| !c.is_whitespace());

        println!("importing account...");
        drive
            .lb
            .import_account(&account_string, None)
            .await
            .unwrap();

        drive.lb.sync(Self::progress()).await.unwrap();

        Ok(())
    }

    pub async fn mount() -> CliResult<()> {
        let drive = Self::init().await;
        drive.prepare_caches().await;
        info!("registering sig handler");

        // capture ctrl_c and try to cleanup
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            umount().await;
            info!("cleaned up, goodbye!");
            exit(0);
        });

        // sync periodically in the background
        let syncer = drive.clone();
        tokio::spawn(async move {
            loop {
                info!("will sync in 5 minutes");
                tokio::time::sleep(Duration::from_secs(3)).await;
                info!("syncing");
                syncer.sync().await;
            }
        });

        // todo have a better port selection strategy
        info!("creating server");
        let listener = NFSTcpListener::bind("127.0.0.1:11111", drive)
            .await
            .unwrap();

        info!("mounting");
        mount();

        info!("ready");
        listener.handle_forever().await.unwrap();
        Ok(())
    }

    pub fn progress() -> Option<Box<dyn Fn(SyncProgress) + Send>> {
        Some(Box::new(|status| println!("{status}")))
    }
}
