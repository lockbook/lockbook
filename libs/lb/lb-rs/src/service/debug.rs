use crate::model::clock;
use crate::model::errors::LbResult;
use crate::service::logging::LOG_FILE;
use crate::{Lb, get_code_version};
use basic_human_duration::ChronoHumanDuration;
use chrono::NaiveDateTime;
use serde::Serialize;
use std::env;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use time::Duration;
use tokio::fs::{self, File};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

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
    pub log_tail: String,
    pub last_panic: String,
}

impl Lb {
    async fn tail_log(&self) -> LbResult<String> {
        let mut path = PathBuf::from(&self.config.writeable_path);
        if path.exists() {
            path.push(LOG_FILE);
            let mut file = File::open(path).await?;
            let size = file.metadata().await?.len();
            let read_amount = 5 * 1024;
            let pos = size.saturating_sub(read_amount);

            let mut buffer = Vec::with_capacity(read_amount as usize);
            file.seek(SeekFrom::Start(pos)).await?;
            file.read_to_end(&mut buffer).await?;
            if self.config.colored_logs {
                // strip colors
                buffer = strip_ansi_escapes::strip(buffer);
            }
            Ok(String::from_utf8_lossy(&buffer).to_string())
        } else {
            Ok("NO LOGS FOUND".to_string())
        }
    }

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

    async fn find_most_recent_panic_log(&self) -> LbResult<String> {
        let dir_path = &self.config.writeable_path;
        let path = Path::new(dir_path);

        let prefix = "panic---";
        let suffix = ".log";
        let timestamp_format = "%Y-%m-%d---%H-%M-%S";

        let mut most_recent_file: Option<String> = None;
        let mut most_recent_time: Option<NaiveDateTime> = None;

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
                    // Compare to find the most recent timestamp
                    match most_recent_time {
                        Some(ref current_most_recent) => {
                            if timestamp > *current_most_recent {
                                most_recent_time = Some(timestamp);
                                most_recent_file = Some(file_name.clone());
                            }
                        }
                        None => {
                            most_recent_time = Some(timestamp);
                            most_recent_file = Some(file_name.clone());
                        }
                    }
                }
            }
        }

        // If we found the most recent file, read its contents
        if let Some(file_name) = most_recent_file {
            let file_path = path.join(file_name);
            let contents = fs::read_to_string(file_path).await?;
            Ok(contents)
        } else {
            Ok(String::default())
        }
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn debug_info(&self, os_info: String) -> LbResult<String> {
        let account = self.get_account()?;

        let arch = env::consts::ARCH;
        let os = env::consts::OS;
        let family = env::consts::FAMILY;

        let (integrity, log_tail, last_synced, last_panic) = tokio::join!(
            self.test_repo_integrity(),
            self.tail_log(),
            self.human_last_synced(),
            self.find_most_recent_panic_log()
        );

        Ok(serde_json::to_string_pretty(&DebugInfo {
            time: self.now(),
            name: account.username.clone(),
            lb_version: get_code_version().into(),
            rust_triple: format!("{arch}.{family}.{os}"),
            server_url: account.api_url.clone(),
            integrity: format!("{integrity:?}"),
            log_tail: log_tail?,
            lb_dir: self.config.writeable_path.clone(),
            last_synced,
            os_info,
            last_panic: last_panic?,
        })?)
    }
}
