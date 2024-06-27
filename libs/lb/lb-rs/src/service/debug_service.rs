use super::api_service::Requester;
use crate::Duration;
use crate::{
    get_code_version, service::log_service::LOG_FILE, CoreState, LbResult, TestRepoError, Warning,
};
use ansi_term::ANSIString;
use basic_human_duration::ChronoHumanDuration;
use lockbook_shared::{clock, document_repo::DocumentService};
use serde::Serialize;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    os::unix::fs::MetadataExt,
    path::PathBuf,
};

#[derive(Serialize)]
pub struct DebugInfo {
    pub time: String,
    pub last_synced: String,
    pub version: String,
    pub name: String,
    pub lb_dir: String,
    pub server_url: String,
    pub log_tail: String,
    pub integrity: Result<Vec<Warning>, TestRepoError>,
}

impl<Client: Requester, Docs: DocumentService> CoreState<Client, Docs> {
    fn tail_log(&self) -> LbResult<String> {
        let mut path = PathBuf::from(self.config.writeable_path);
        if path.exists() {
            path.push(LOG_FILE);
            let mut file = File::open(path)?;
            let size = file.metadata()?.size();
            let read_amount = 10 * 1024;
            let pos = if read_amount > size { 0 } else { size - read_amount };

            let mut buffer = Vec::with_capacity(read_amount as usize);
            file.seek(SeekFrom::Start(pos))?;
            file.read_to_end(&mut buffer);
            let raw_log = String::from_utf8_lossy(&buffer).to_string();
            if self.config.colored_logs {
                // strip colors
                Ok(ANSIString::from(raw_log).to_string())
            } else {
                Ok(raw_log)
            }
        } else {
            Ok("NO LOGS FOUND".to_string())
        }
    }

    fn human_last_synced(&self) -> String {
        let last_synced = *self.db.last_synced.get().unwrap_or(&0);

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

    pub(crate) fn debug_info(&self) -> LbResult<String> {
        let account = self.get_account()?;

        Ok(serde_json::to_string_pretty(&DebugInfo {
            version: get_code_version().into(),
            name: account.username,
            server_url: account.api_url,
            integrity: self.test_repo_integrity(),
            log_tail: self.tail_log()?,
            lb_dir: self.config.writeable_path,
            time: self.now(),
            last_synced: self.human_last_synced(),
        })?)
    }
}
