use cli_rs::cli_error::{CliError, CliResult};
use lb_rs::model::core_config::Config;
use lb_rs::Lb;
use lb_sync_dir::SyncDirConfig;
use std::path::PathBuf;
use std::time::Duration;

use crate::ensure_account;

#[tokio::main]
pub async fn run(
    lockbook_folder: String,
    local_dir: String,
    pull_interval: Option<String>,
    no_watch: bool,
    once: bool,
) -> CliResult<()> {
    let lb = Lb::init(Config::cli_config("cli"))
        .await
        .map_err(|err| CliError::from(err.to_string()))?;
    ensure_account(&lb)?;

    let pull_interval = match pull_interval {
        Some(s) => parse_duration(&s)?,
        None => Duration::from_secs(5),
    };

    let config = SyncDirConfig {
        lockbook_folder,
        local_dir: PathBuf::from(local_dir),
        pull_interval,
        watch: !no_watch,
        once,
    };

    if config.once {
        lb_sync_dir::run_once(&lb, &config)
            .await
            .map_err(|e| CliError::from(e.to_string()))?;
    } else {
        lb_sync_dir::run(&lb, &config)
            .await
            .map_err(|e| CliError::from(e.to_string()))?;
    }

    Ok(())
}

fn parse_duration(s: &str) -> CliResult<Duration> {
    let s = s.trim();
    if let Some(secs) = s.strip_suffix('s') {
        secs.parse::<u64>()
            .map(Duration::from_secs)
            .map_err(|_| CliError::from(format!("invalid duration: {s}")))
    } else if let Some(ms) = s.strip_suffix("ms") {
        ms.parse::<u64>()
            .map(Duration::from_millis)
            .map_err(|_| CliError::from(format!("invalid duration: {s}")))
    } else if let Some(m) = s.strip_suffix('m') {
        m.parse::<u64>()
            .map(|v| Duration::from_secs(v * 60))
            .map_err(|_| CliError::from(format!("invalid duration: {s}")))
    } else {
        s.parse::<u64>()
            .map(Duration::from_secs)
            .map_err(|_| CliError::from(format!("invalid duration: {s} (expected e.g. 5s, 500ms, 1m)")))
    }
}
