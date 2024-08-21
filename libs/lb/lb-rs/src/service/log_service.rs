use crate::model::errors::core_err_unexpected;
use crate::{Config, LbResult};
use std::{backtrace, env, panic};
use tracing::metadata::LevelFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::{filter, fmt, prelude::*, Layer};

pub static LOG_FILE: &str = "lockbook.log";

pub fn init(config: &Config) -> LbResult<()> {
    if config.logs {
        let lockbook_log_level = env::var("LOG_LEVEL")
            .ok()
            .and_then(|s| s.as_str().parse().ok())
            .unwrap_or(LevelFilter::DEBUG);

        let subscriber = tracing_subscriber::Registry::default()
            .with(
                fmt::Layer::new()
                    .with_span_events(FmtSpan::ACTIVE)
                    .with_ansi(config.colored_logs)
                    .with_target(true)
                    .with_writer(tracing_appender::rolling::never(&config.writeable_path, LOG_FILE))
                    .with_filter(lockbook_log_level)
                    .with_filter(filter::filter_fn(|metadata| {
                        metadata.target().starts_with("lb_rs")
                            || metadata.target().starts_with("dbrs")
                    })),
            )
            .with(
                fmt::Layer::new()
                    .with_ansi(false)
                    .with_span_events(FmtSpan::CLOSE)
                    .with_filter(filter::filter_fn(|metadata| {
                        metadata.target().starts_with("lb_fs")
                            || metadata.target().contains("svg_editor")
                    })),
            );

        tracing::subscriber::set_global_default(subscriber).map_err(core_err_unexpected)?;
        panic_capture();
    }
    Ok(())
}

fn panic_capture() {
    panic::set_hook(Box::new(|panic_info| {
        tracing::error!("panic detected: {panic_info} {}", backtrace::Backtrace::force_capture());
        eprintln!(
            "panic detected and logged: {panic_info} {}",
            backtrace::Backtrace::force_capture()
        );
    }));
}
