use super::api_service::Requester;
use crate::Duration;
use crate::{get_code_version, service::log_service::LOG_FILE, CoreState, LbResult};
use basic_human_duration::ChronoHumanDuration;
use crate::shared::{clock, document_repo::DocumentService};
use serde::Serialize;
use std::env;
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
};

#[derive(Serialize)]
pub struct DebugInfo {
    pub time: String,
    pub name: String,
    pub last_synced: String,
    pub lb_version: String,
    pub rust_tripple: String,
    pub os_info: String,
    pub lb_dir: String,
    pub server_url: String,
    pub integrity: String,
    pub log_tail: String,
}

impl<Client: Requester, Docs: DocumentService> CoreState<Client, Docs> {
    fn tail_log(&self) -> LbResult<String> {
        let mut path = PathBuf::from(&self.config.writeable_path);
        if path.exists() {
            path.push(LOG_FILE);
            let mut file = File::open(path)?;
            let size = file.metadata()?.len();
            let read_amount = 20 * 1024;
            let pos = if read_amount > size { 0 } else { size - read_amount };

            let mut buffer = Vec::with_capacity(read_amount as usize);
            file.seek(SeekFrom::Start(pos))?;
            file.read_to_end(&mut buffer)?;
            if self.config.colored_logs {
                // strip colors
                buffer = strip_ansi_escapes::strip(buffer);
            }
            Ok(String::from_utf8_lossy(&buffer).to_string())
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

    pub(crate) fn debug_info(&self, os_info: String) -> LbResult<String> {
        let account = self.get_account()?;

        let arch = env::consts::ARCH;
        let os = env::consts::OS;
        let family = env::consts::FAMILY;

        Ok(serde_json::to_string_pretty(&DebugInfo {
            time: self.now(),
            name: account.username.clone(),
            lb_version: get_code_version().into(),
            rust_tripple: format!("{arch}.{family}.{os}"),
            server_url: account.api_url.clone(),
            integrity: format!("{:?}", self.test_repo_integrity()),
            log_tail: self.tail_log()?,
            lb_dir: self.config.writeable_path.clone(),
            last_synced: self.human_last_synced(),
            os_info,
        })?)
    }
}
