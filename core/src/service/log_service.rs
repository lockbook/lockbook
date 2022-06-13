use crate::model::errors::core_err_unexpected;
use crate::CoreError;
use std::env;
use std::path::Path;
use tracing::metadata::LevelFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::{filter, fmt, prelude::*};

static LOG_FILE: &str = "lockbook.log";

pub fn init<P: AsRef<Path>>(log_path: P) -> Result<(), CoreError> {
    let lockbook_log_level = env::var("LOG_LEVEL")
        .ok()
        .and_then(|s| s.as_str().parse().ok())
        .unwrap_or(LevelFilter::DEBUG);

    let subscriber = tracing_subscriber::Registry::default().with(
        fmt::Layer::new()
            .with_span_events(FmtSpan::ACTIVE)
            .with_target(false)
            .with_writer(tracing_appender::rolling::never(&log_path, LOG_FILE))
            .with_filter(lockbook_log_level)
            .with_filter(filter::filter_fn(|metadata| {
                metadata.target().starts_with("lockbook_core")
            })),
    );

    tracing::subscriber::set_global_default(subscriber).map_err(core_err_unexpected)?;

    Ok(())
}
