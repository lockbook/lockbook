use crate::Lb;
use crate::model::clock;
use crate::model::errors::LbResult;
use crate::service::lb_id::LbID;
use basic_human_duration::ChronoHumanDuration;
use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use time::Duration;

#[cfg(not(target_family = "wasm"))]
use std::path::Path;

#[cfg(not(target_family = "wasm"))]
use std::env;

#[cfg(not(target_family = "wasm"))]
use chrono::{DateTime, Local, TimeZone};

#[cfg(not(target_family = "wasm"))]
use crate::get_code_version;

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
    async fn collect_panics(&self, populate_content: bool) -> LbResult<Vec<PanicInfo>> {
        let mut panics = vec![];
        let mut iter = iter_panic_files(&self.config.writeable_path).await?;
        while let Some(entry) = iter.next().await? {
            let content = if populate_content { entry.content().await? } else { String::new() };
            panics.push(PanicInfo { time: entry.time, file_path: entry.file_path, content });
        }
        panics.sort_by_key(|panic| std::cmp::Reverse(panic.time));

        Ok(panics)
    }

    /// When we last crashed, to the second, if a panic log survives.
    #[cfg(not(target_family = "wasm"))]
    pub async fn recent_panic(&self) -> LbResult<Option<DateTime<Local>>> {
        recent_panic(&self.config.writeable_path).await
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    #[cfg(not(target_family = "wasm"))]
    pub async fn write_panic_to_file(&self, error_header: String, bt: String) -> LbResult<String> {
        let file_name = generate_panic_filename(&self.config.writeable_path);
        let content = generate_panic_content(&error_header, &bt);

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
    pub async fn debug_info(&self, os_info: String, check_docs: bool) -> LbResult<DebugInfo> {
        let account = self.get_account()?;

        let arch = env::consts::ARCH;
        let os = env::consts::OS;
        let family = env::consts::FAMILY;

        let (integrity, last_synced, panics, lb_id) = tokio::join!(
            self.test_repo_integrity(check_docs),
            self.human_last_synced(),
            self.collect_panics(true),
            self.lb_id()
        );

        let panics = panics?.into_iter().map(|panic| panic.content).collect();

        let mut status = self.status().await;
        status.space_used = None;
        let status = format!("{status:?}");
        let is_syncing = self.syncer.try_lock().is_ok();

        Ok(DebugInfo {
            time: self.now(),
            name: account.username.clone(),
            lb_version: get_code_version().into(),
            lb_id: lb_id?,
            rust_triple: format!("{arch}.{family}.{os}"),
            server_url: account.api_url.clone(),
            integrity: format!(
                "{} {integrity:?}",
                if check_docs { "" } else { "doc checks skipped" }
            ),
            lb_dir: self.config.writeable_path.clone(),
            last_synced,
            os_info,
            status,
            is_syncing,
            panics,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanicInfo {
    pub time: NaiveDateTime,
    pub file_path: PathBuf,
    pub content: String,
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
            if !entry.file_type().await?.is_file() {
                continue;
            }
            if is_metrickit_filename(&file_name) {
                // This is the receipt time. The diagnostic itself retains Apple's
                // reporting interval and the version of the app that crashed.
                let received: chrono::DateTime<Local> = entry.metadata().await?.modified()?.into();
                return Ok(Some(PanicFile {
                    time: received.naive_local(),
                    file_path: self.base_path.join(file_name),
                }));
            }
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
    fn is_metrickit(&self) -> bool {
        self.file_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_metrickit_filename)
    }

    pub async fn content(&self) -> LbResult<String> {
        let contents = fs::read_to_string(&self.file_path).await?;
        let time_label = if self.is_metrickit() { "received" } else { "time" };
        Ok(format!("{time_label}: {}: contents: {}", self.time, contents))
    }
}

#[cfg(not(target_family = "wasm"))]
fn is_metrickit_filename(name: &str) -> bool {
    name.strip_prefix("metrickit---")
        .and_then(|name| name.strip_suffix(".log"))
        .is_some_and(|id| id.len() == 64 && id.bytes().all(|b| b.is_ascii_hexdigit()))
}

#[cfg(not(target_family = "wasm"))]
async fn recent_panic(path: &str) -> LbResult<Option<DateTime<Local>>> {
    let mut iter = iter_panic_files(path).await?;
    let mut latest = None;
    while let Some(panic) = iter.next().await? {
        // Delayed MetricKit delivery must not disable workspace restoration as
        // though the app had just crashed. Its receipt time is not a crash time.
        if panic.is_metrickit() {
            continue;
        }
        latest = latest.max(Some(panic.time));
    }
    Ok(latest.and_then(|time| Local.from_local_datetime(&time).single()))
}

#[cfg(not(target_family = "wasm"))]
pub(crate) async fn iter_panic_files(path: &str) -> LbResult<PanicFiles> {
    let base_path = Path::new(path).to_path_buf();
    let entries = fs::read_dir(&base_path).await?;
    Ok(PanicFiles { entries, base_path })
}

/// Fingerprint report filenames, lengths and modification times without reading
/// every backtrace on every sync. This also detects delayed MetricKit reports,
/// multiple imports at the same time, and appends to a Rust panic log.
/// Negative values distinguish fingerprints from legacy timestamp markers.
#[cfg(not(target_family = "wasm"))]
pub(crate) async fn crash_report_marker(path: &str) -> LbResult<i64> {
    use sha2::{Digest, Sha256};
    use std::time::UNIX_EPOCH;

    let mut iter = iter_panic_files(path).await?;
    let mut reports = Vec::new();
    while let Some(entry) = iter.next().await? {
        let metadata = fs::metadata(&entry.file_path).await?;
        let modified = metadata
            .modified()?
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        reports.push((entry.file_path, metadata.len(), modified.as_nanos()));
    }
    if reports.is_empty() {
        return Ok(0);
    }
    reports.sort();
    let mut hash = Sha256::new();
    hash.update(b"lockbook-crash-reports-v1");
    for (path, len, modified) in reports {
        let name = path.file_name().unwrap().to_string_lossy();
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name.as_bytes());
        hash.update(len.to_le_bytes());
        hash.update(modified.to_le_bytes());
    }
    let digest = hash.finalize();
    Ok(i64::from_le_bytes(digest[..8].try_into().unwrap()) | i64::MIN)
}

pub fn generate_panic_filename(path: &str) -> String {
    let timestamp = chrono::Local::now().format("%Y-%m-%d---%H-%M-%S");
    format!("{path}/panic---{timestamp}.log")
}

pub fn generate_panic_content(panic_info: &str, bt: &str) -> String {
    format!("INFO: {panic_info}\nBT: {bt}")
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use std::time::{Duration as StdDuration, UNIX_EPOCH};

    fn metrickit_name(id: char) -> String {
        format!("metrickit---{}.log", id.to_string().repeat(64))
    }

    #[tokio::test]
    async fn collects_rust_and_metrickit_reports_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        fs::write(dir.path().join("panic---2026-01-01---01-02-03.log"), "INFO: Rust")
            .await
            .unwrap();
        fs::write(dir.path().join(metrickit_name('a')), "INFO: MetricKit\n{\"signal\":5}")
            .await
            .unwrap();
        for ignored in
            ["lockbook.log", "panic---invalid.log", "metrickit---invalid.log", ".atomic-write"]
        {
            fs::write(dir.path().join(ignored), "not a crash")
                .await
                .unwrap();
        }
        fs::create_dir(dir.path().join(metrickit_name('b')))
            .await
            .unwrap();

        let mut iter = iter_panic_files(path).await.unwrap();
        let mut reports = Vec::new();
        while let Some(report) = iter.next().await.unwrap() {
            reports.push(report.content().await.unwrap());
        }
        assert_eq!(reports.len(), 2);
        assert!(
            reports
                .iter()
                .any(|r| r.starts_with("time: 2026-01-01 01:02:03: contents: INFO: Rust"))
        );
        assert!(
            reports
                .iter()
                .any(|r| r.starts_with("received:") && r.contains("\"signal\":5"))
        );
    }

    #[tokio::test]
    async fn marker_detects_batched_delayed_reports_and_rust_appends() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        assert_eq!(crash_report_marker(path).await.unwrap(), 0);
        let rust = dir.path().join("panic---2026-01-01---01-02-03.log");
        fs::write(&rust, "INFO: Rust").await.unwrap();
        let initial = crash_report_marker(path).await.unwrap();
        assert!(initial < 0, "fingerprints must differ from legacy timestamps");
        assert_eq!(crash_report_marker(path).await.unwrap(), initial);

        let mut previous = initial;
        for id in ['a', 'b'] {
            let file = dir.path().join(metrickit_name(id));
            fs::write(&file, "INFO: MetricKit").await.unwrap();
            // Both reports arrive with the same mtime, older than the Rust panic.
            std::fs::File::open(file)
                .unwrap()
                .set_modified(UNIX_EPOCH + StdDuration::from_secs(1))
                .unwrap();
            let marker = crash_report_marker(path).await.unwrap();
            assert_ne!(marker, previous);
            previous = marker;
        }
        let modified = std::fs::metadata(&rust).unwrap().modified().unwrap();
        fs::write(&rust, "INFO: Rust\nINFO: another panic in the same second")
            .await
            .unwrap();
        std::fs::File::open(&rust)
            .unwrap()
            .set_modified(modified)
            .unwrap();
        let appended = crash_report_marker(path).await.unwrap();
        assert_ne!(appended, previous);
        fs::write(dir.path().join("lockbook.log"), "ordinary logging")
            .await
            .unwrap();
        assert_eq!(crash_report_marker(path).await.unwrap(), appended);
    }

    #[tokio::test]
    async fn delayed_metrickit_reports_do_not_trigger_crash_loop_recovery() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_str().unwrap();
        fs::write(dir.path().join(metrickit_name('a')), "INFO: old MetricKit crash")
            .await
            .unwrap();
        assert_eq!(recent_panic(path).await.unwrap(), None);
        fs::write(dir.path().join("panic---2026-01-02---01-02-03.log"), "INFO: last crash")
            .await
            .unwrap();
        fs::write(dir.path().join("panic---2026-01-01---01-02-03.log"), "INFO: older crash")
            .await
            .unwrap();
        let latest = recent_panic(path).await.unwrap().unwrap();
        assert_eq!(latest.naive_local().to_string(), "2026-01-02 01:02:03");
    }
}
