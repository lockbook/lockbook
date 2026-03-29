use crate::cache::FileEntry;
use crate::fs_impl::Drive;
use crate::mount::{mount, umount};
use cli_rs::cli_error::{CliError, CliResult};
use lb_rs::model::core_config::Config;
use lb_rs::model::errors::Unexpected;
use lb_rs::service::events::{Actor, Event};
use lb_rs::{Lb, Uuid};
use nfs3_server::tcp::{NFSTcp, NFSTcpListener};
use std::io;
use std::io::IsTerminal;
use std::process::exit;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;
use tracing::{debug, error, info};

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

        logger::init();

        let root = lb.root().await.map(|file| file.id).unwrap_or(Uuid::nil());

        let data = Arc::default();

        let fs = Self { lb, root, data };
        fs.clone().monitor_lb().await;

        fs
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

        drive.lb.sync().await.unwrap();

        Ok(())
    }

    pub async fn mount() -> CliResult<()> {
        let drive = Self::init().await;
        drive.lb.sync().await.unwrap();
        drive.fill_cache().await;
        info!("registering sig handler");

        // capture ctrl_c and try to cleanup
        tokio::spawn(async move {
            tokio::signal::ctrl_c().await.unwrap();
            let mut unmount_success = umount().await;
            while !unmount_success {
                error!("unmount failed, please close any apps using lb-fs! Retrying in 1s.");
                time::sleep(Duration::from_secs(1)).await;
                unmount_success = umount().await;
            }
            info!("cleaned up, goodbye!");
            exit(0);
        });

        // sync periodically in the background
        let syncer = drive.clone();
        tokio::spawn(async move {
            loop {
                info!("will sync in 30 seconds");
                tokio::time::sleep(Duration::from_secs(30)).await;
                info!("syncing");
                syncer.lb.sync().await.map_unexpected().log_and_ignore();
            }
        });

        // monitor changes to lb
        let event_handler = drive.clone();
        tokio::spawn(async move {
            let mut events = event_handler.lb.subscribe();
            loop {
                let event = events.recv().await.unwrap();

                // todo: this is the last thing that needs to be fleshed out for lb-fs to be fully
                // embedded into desktop clients:
                // there needs to be a more nuanced concept of Actor, so that we can respond to
                // changes other clients made without reacting to changes that we made ourselves
                //
                // perhaps it would be nice to additionally integrate the status of lb-fs in status
                // generally. Maybe it would be best for workspace to orchestrate it? Maybe have a
                // tab dedicated to the status of the virtual file system mount
                //
                // maybe that works nicely with tab persistence too. can gate to beta_users pretty
                // easily. Maybe can have a special filename that lets people test an early version
                match event {
                    Event::MetadataChanged(Actor::Sync) => event_handler.fill_cache().await,
                    Event::DocumentWritten(dirty_id, Actor::Sync) => {
                        let file = event_handler.lb.get_file_by_id(dirty_id).await.unwrap();
                        let size = if file.is_document() {
                            event_handler
                                .lb
                                .read_document(dirty_id, false)
                                .await
                                .unwrap()
                                .len()
                        } else {
                            0
                        };

                        let mut entry = FileEntry::from_file(file, size as u64);

                        let now = FileEntry::now();

                        entry.fattr.mtime = now;
                        entry.fattr.ctime = now;

                        event_handler
                            .data
                            .lock()
                            .await
                            .insert(entry.file.id.into(), entry);
                    }
                    _ => {}
                }
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

    async fn monitor_lb(self) {
        tokio::spawn(async move {
            let mut sub = self.lb.subscribe();
            loop {
                if let Event::Sync(sync_increment) = sub.recv().await.unwrap() {
                    debug!("syncing: {sync_increment:?}")
                }
            }
        });
    }
}
