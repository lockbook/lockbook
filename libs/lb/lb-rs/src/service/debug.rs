use crate::model::clock;
use crate::model::errors::LbResult;
use crate::{Lb, get_code_version};
use basic_human_duration::ChronoHumanDuration;
use chrono::NaiveDateTime;
use serde::Serialize;
use std::env;
use std::path::Path;
use std::sync::atomic::Ordering;
use time::Duration;
use tokio::fs::{self};

#[derive(Serialize)]
pub struct DebugInfo {
    pub time: String,
    pub name: String,
    pub last_synced: String,
    pub lb_version: String,
    pub rust_triple: String,
    pub os_info: String,
    pub lb_dir: String,
    pub server_url: String,
    pub integrity: String,
    pub is_syncing: bool,
    pub status: String,
    pub panics: Vec<String>,
}

impl Lb {
    async fn human_last_synced(&self) -> String {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let last_synced = *db.last_synced.get().unwrap_or(&0);

        if last_synced != 0 {
            Duration::milliseconds(clock::get_time().0 - last_synced)
                .format_human()
                .to_string()
        } else {
            "never".to_string()
        }
    }

    fn now(&self) -> String {
        let now = chrono::Local::now();
        now.format("%Y-%m-%d %H:%M:%S %Z").to_string()
    }

    async fn collect_panics(&self) -> LbResult<Vec<String>> {
        let mut panics = vec![];

        let dir_path = &self.config.writeable_path;
        let path = Path::new(dir_path);

        let prefix = "panic---";
        let suffix = ".log";
        let timestamp_format = "%Y-%m-%d---%H-%M-%S";

        let mut entries = fs::read_dir(path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let file_name = entry.file_name().into_string().unwrap_or_default();

            // Check if the filename matches the expected format
            if file_name.starts_with(prefix) && file_name.ends_with(suffix) {
                // Extract the timestamp portion from the filename
                let timestamp_str = &file_name[prefix.len()..file_name.len() - suffix.len()];

                // Parse the timestamp
                if let Ok(timestamp) =
                    NaiveDateTime::parse_from_str(timestamp_str, timestamp_format)
                {
                    let file_path = path.join(file_name);
                    let contents = fs::read_to_string(file_path).await?;
                    let contents = format!("time: {timestamp}: contents: {contents}");
                    panics.push(contents);
                }
            }
        }
        Ok(panics)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn debug_info(&self, os_info: String) -> LbResult<String> {
        let account = self.get_account()?;

        let arch = env::consts::ARCH;
        let os = env::consts::OS;
        let family = env::consts::FAMILY;

        let (integrity, last_synced, panics) = tokio::join!(
            self.test_repo_integrity(),
            self.human_last_synced(),
            self.collect_panics()
        );

        let mut status = self.status().await;
        status.space_used = None;
        let status = format!("{status:?}");
        let is_syncing = self.syncing.load(Ordering::Relaxed);

        Ok(serde_json::to_string_pretty(&DebugInfo {
            time: self.now(),
            name: account.username.clone(),
            lb_version: get_code_version().into(),
            rust_triple: format!("{arch}.{family}.{os}"),
            server_url: account.api_url.clone(),
            integrity: format!("{integrity:?}"),
            lb_dir: self.config.writeable_path.clone(),
            last_synced,
            os_info,
            status,
            is_syncing,
            panics: panics?,
        })?)
    }
}
