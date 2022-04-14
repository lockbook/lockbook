use std::env;
use std::ops::Deref;
use std::path::Path;
use std::str::FromStr;
use std::sync::atomic::AtomicBool;

use tracing::metadata::LevelFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::{filter, fmt, prelude::*};

use crate::external_interface::static_state::LOG_INITED;
use crate::model::errors::core_err_unexpected;
use crate::{unexpected_only, CoreError};

static LOG_FILE: &str = "lockbook.log";

pub fn init<P: AsRef<Path>>(log_path: P) -> Result<(), CoreError> {
    let lockbook_log_level = env::var("LOG_LEVEL")
        .ok()
        .and_then(|s| LevelFilter::from_str(s.as_str()).ok())
        .unwrap_or(LevelFilter::DEBUG);

    let subscriber = tracing_subscriber::Registry::default().with(
        fmt::Layer::new()
            .with_span_events(FmtSpan::ACTIVE)
            .with_target(false)
            .with_writer(tracing_appender::rolling::never(log_path, LOG_FILE))
            .with_filter(lockbook_log_level)
            .with_filter(filter::filter_fn(|metadata| metadata.target() == "lockbook_core")),
    );

    tracing::subscriber::set_global_default(subscriber).map_err(core_err_unexpected)?;

    Ok(())
}
