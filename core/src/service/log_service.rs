use std::env;
use std::path::Path;
use std::str::FromStr;

use tracing::metadata::LevelFilter;
use tracing_subscriber::{fmt, prelude::*};

use crate::model::errors::core_err_unexpected;
use crate::CoreError;

static LOG_FILE: &str = "lockbook.log";

pub fn init(log_path: &Path) -> Result<(), CoreError> {
    let lockbook_log_level = env::var("LOG_LEVEL")
        .ok()
        .and_then(|s| LevelFilter::from_str(s.as_str()).ok())
        .unwrap_or(LevelFilter::DEBUG);

    let subscriber = tracing_subscriber::Registry::default()
        .with(
            fmt::Layer::new()
                .with_writer(tracing_appender::rolling::never(log_path, LOG_FILE))
                .with_filter(lockbook_log_level),
        )
        .with(
            fmt::Layer::new()
                .with_writer(std::io::stdout)
                .with_filter(LevelFilter::WARN),
        );

    tracing::subscriber::set_global_default(subscriber).map_err(core_err_unexpected)?;

    Ok(())
}
