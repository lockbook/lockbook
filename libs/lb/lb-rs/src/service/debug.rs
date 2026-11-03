use crate::Lb;
use crate::model::clock;
use crate::model::core_config::Config;
use crate::model::errors::LbResult;
use crate::service::lb_id::LbID;
use basic_human_duration::ChronoHumanDuration;
use serde::{Deserialize, Serialize};
use time::Duration;

#[cfg(not(target_family = "wasm"))]
use chrono::NaiveDateTime;

#[cfg(not(target_family = "wasm"))]
use std::path::{Path, PathBuf};

#[cfg(not(target_family = "wasm"))]
use std::env;

#[cfg(not(target_family = "wasm"))]
use chrono::{DateTime, Local, TimeZone};

#[cfg(not(target_family = "wasm"))]
use tokio::fs::{self, OpenOptions};

#[cfg(not(target_family = "wasm"))]
use tokio::io::AsyncWriteExt;

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct DebugInfo {
    pub lb_id: LbID,
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

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct PanicReport {
    pub time: String,
    pub info: String,
    pub config: Option<Config>,
    pub backtrace: String,
}

pub trait DebugInfoDisplay {
    fn to_string(&self) -> String;
}

impl DebugInfoDisplay for LbResult<DebugInfo> {
    fn to_string(&self) -> String {
        match self {
            Ok(debug_info) => serde_json::to_string_pretty(debug_info).unwrap_or_default(),
            Err(err) => format!("Error retrieving debug info: {:?}", err),
        }
    }
}

impl Lb {
    async fn human_last_synced(&self) -> String {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let last_synced = *db.last_synced.as_ref().unwrap_or(&0);

        if last_synced != 0 {
            Duration::milliseconds(clock::get_time().0 - last_synced)
                .format_human()
                .to_string()
        } else {
            "never".to_string()
        }
    }

    async fn lb_id(&self) -> LbResult<LbID> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let lb_id = if let Some(id) = db.id.as_ref().copied() {
            id
        } else {
            let new_id = LbID::generate();
            db.id.replace(new_id)?;
            new_id
        };

        tx.end();

        Ok(lb_id)
    }

    fn now(&self) -> String {
        let now = chrono::Local::now();
        now.format("%Y-%m-%d %H:%M:%S %Z").to_string()
    }

    #[cfg(not(target_family = "wasm"))]
    async fn collect_panics(&self) -> LbResult<Vec<PanicReport>> {
        let mut files = vec![];
        let mut iter = iter_panic_files(&self.config.writeable_path).await?;
        while let Some(entry) = iter.next().await? {
            files.push(entry);
        }
        files.sort_by_key(|file| std::cmp::Reverse(file.time));

        let mut panics = vec![];
        for file in files {
            panics.extend(file.reports().await?);
        }
        Ok(panics)
    }

    /// When we last crashed, to the second, if a panic log survives.
    #[cfg(not(target_family = "wasm"))]
    pub async fn recent_panic(&self) -> LbResult<Option<DateTime<Local>>> {
        let mut latest = None;
        let mut iter = iter_panic_files(&self.config.writeable_path).await?;
        while let Some(entry) = iter.next().await? {
            latest = latest.max(Some(entry.time));
        }
        Ok(latest.and_then(|time| Local.from_local_datetime(&time).single()))
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    #[cfg(not(target_family = "wasm"))]
    pub async fn write_panic_to_file(&self, error_header: String, bt: String) -> LbResult<String> {
        let file_name = generate_panic_filename(&self.config.writeable_path);
        let content = generate_panic_content(&self.config, &error_header, &bt);

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_name)
            .await?;

        file.write_all(content.as_bytes()).await?;

        Ok(file_name)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    #[cfg(not(target_family = "wasm"))]
    pub async fn debug_info(&self, check_docs: bool) -> LbResult<DebugInfo> {
        let account = self.get_account()?;

        let arch = env::consts::ARCH;
        let os = env::consts::OS;
        let family = env::consts::FAMILY;

        let (integrity, last_synced, panics, lb_id) = tokio::join!(
            self.test_repo_integrity(check_docs),
            self.human_last_synced(),
            self.collect_panics(),
            self.lb_id()
        );

        let panics = panics?
            .iter()
            .map(|panic| serde_json::to_string(panic).unwrap_or_default())
            .collect();

        let mut status = self.status().await;
        status.space_used = None;
        let status = format!("{status:?}");
        let is_syncing = self.syncer.try_lock().is_err();

        Ok(DebugInfo {
            lb_id: lb_id?,
            time: self.now(),
            name: account.username.clone(),
            last_synced,
            lb_version: self.config.lb_version.clone(),
            rust_triple: format!("{arch}.{family}.{os}"),
            os_info: self.config.os_version.clone().unwrap_or_default(),
            lb_dir: self.config.writeable_path.clone(),
            server_url: account.api_url.clone(),
            integrity: format!(
                "{} {integrity:?}",
                if check_docs { "" } else { "doc checks skipped" }
            ),
            is_syncing,
            status,
            panics,
        })
    }
}

#[cfg(not(target_family = "wasm"))]
pub struct PanicFiles {
    entries: tokio::fs::ReadDir,
    base_path: PathBuf,
}

#[cfg(not(target_family = "wasm"))]
pub struct PanicFile {
    pub time: NaiveDateTime,
    pub file_path: PathBuf,
}

#[cfg(not(target_family = "wasm"))]
impl PanicFiles {
    pub async fn next(&mut self) -> LbResult<Option<PanicFile>> {
        const PREFIX: &str = "panic---";
        const SUFFIX: &str = ".log";
        const TIMESTAMP_FORMAT: &str = "%Y-%m-%d---%H-%M-%S";

        while let Some(entry) = self.entries.next_entry().await? {
            let file_name = entry.file_name().into_string().unwrap_or_default();
            if !file_name.starts_with(PREFIX) || !file_name.ends_with(SUFFIX) {
                continue;
            }
            let timestamp_str = &file_name[PREFIX.len()..file_name.len() - SUFFIX.len()];
            let Ok(time) = NaiveDateTime::parse_from_str(timestamp_str, TIMESTAMP_FORMAT) else {
                continue;
            };
            let file_path = self.base_path.join(file_name);
            return Ok(Some(PanicFile { time, file_path }));
        }
        Ok(None)
    }
}

#[cfg(not(target_family = "wasm"))]
impl PanicFile {
    pub async fn reports(&self) -> LbResult<Vec<PanicReport>> {
        let contents = fs::read_to_string(&self.file_path).await?;
        if contents.trim_start().starts_with('{') {
            let reports = serde_json::Deserializer::from_str(&contents)
                .into_iter::<PanicReport>()
                .collect::<Result<Vec<_>, _>>();
            if let Ok(mut reports) = reports {
                reports.reverse();
                return Ok(reports);
            }
        }

        let contents = contents.strip_prefix("INFO: ").unwrap_or(&contents);
        let (info, backtrace) = contents.split_once("\nBT: ").unwrap_or((contents, ""));
        Ok(vec![PanicReport {
            time: self.time.format("%Y-%m-%d %H:%M:%S").to_string(),
            info: info.to_owned(),
            backtrace: backtrace.to_owned(),
            config: None,
        }])
    }
}

#[cfg(not(target_family = "wasm"))]
pub(crate) async fn iter_panic_files(path: &str) -> LbResult<PanicFiles> {
    let base_path = Path::new(path).to_path_buf();
    let entries = fs::read_dir(&base_path).await?;
    Ok(PanicFiles { entries, base_path })
}

/// Millisecond UTC timestamp of the most recent panic file on disk, if any.
#[cfg(not(target_family = "wasm"))]
pub(crate) async fn latest_panic_time(path: &str) -> LbResult<Option<i64>> {
    let mut iter = iter_panic_files(path).await?;
    let mut max: Option<i64> = None;
    while let Some(entry) = iter.next().await? {
        let new_ts = entry.time.and_utc().timestamp_millis();
        match &mut max {
            Some(ts) => {
                if new_ts > *ts {
                    *ts = new_ts;
                }
            }
            None => max = Some(new_ts),
        }
    }
    Ok(max)
}

pub fn generate_panic_filename(path: &str) -> String {
    let timestamp = chrono::Local::now().format("%Y-%m-%d---%H-%M-%S");
    format!("{path}/panic---{timestamp}.log")
}

pub fn generate_panic_content(config: &Config, panic_info: &str, bt: &str) -> String {
    let report = PanicReport {
        time: chrono::Local::now()
            .format("%Y-%m-%d %H:%M:%S %Z")
            .to_string(),
        info: panic_info.to_owned(),
        backtrace: bt.to_owned(),
        config: Some(config.clone()),
    };
    let mut content = serde_json::to_string(&report).unwrap_or_default();
    content.push('\n');
    content
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[tokio::test]
    async fn reads_legacy_and_json_panic_files() {
        let directory = tempdir().unwrap();
        let config = Config { client_name: "tests".into(), ..Default::default() };

        std::fs::write(
            directory.path().join("panic---2026-01-01---10-00-00.log"),
            "INFO: MetricKit crash in app.lockbook (version 1)\nBT: {\n  \"frames\": []\n}\n",
        )
        .unwrap();
        std::fs::write(
            directory.path().join("panic---2026-01-02---10-00-00.log"),
            generate_panic_content(&config, "first", "bt 1")
                + &generate_panic_content(&config, "second", "bt 2"),
        )
        .unwrap();

        let mut files = vec![];
        let mut iter = iter_panic_files(directory.path().to_str().unwrap())
            .await
            .unwrap();
        while let Some(file) = iter.next().await.unwrap() {
            files.push(file);
        }
        files.sort_by_key(|file| file.time);

        let legacy = files[0].reports().await.unwrap();
        assert_eq!(legacy.len(), 1);
        assert_eq!(legacy[0].time, "2026-01-01 10:00:00");
        assert_eq!(legacy[0].info, "MetricKit crash in app.lockbook (version 1)");
        assert_eq!(legacy[0].backtrace, "{\n  \"frames\": []\n}\n");
        assert_eq!(legacy[0].config, None);

        let current = files[1].reports().await.unwrap();
        assert_eq!(
            current.iter().map(|r| r.info.as_str()).collect::<Vec<_>>(),
            ["second", "first"]
        );
        assert_eq!(current[0].backtrace, "bt 2");
        assert_eq!(current[0].config.as_ref(), Some(&config));
    }
}
