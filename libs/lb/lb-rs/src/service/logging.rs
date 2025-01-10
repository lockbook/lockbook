use crate::model::errors::{core_err_unexpected, LbResult};
use crate::Config;
use chrono::Local;
use std::backtrace::Backtrace;
use std::{env, fs, panic};
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

        let mut layers = Vec::with_capacity(2);

        layers.push(
            fmt::Layer::new()
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                .with_ansi(config.colored_logs)
                .with_target(true)
                .with_writer(tracing_appender::rolling::never(&config.writeable_path, LOG_FILE))
                .with_filter(lockbook_log_level)
                .with_filter(filter::filter_fn(|metadata| {
                    metadata.target().starts_with("lb_rs")
                        || metadata.target().starts_with("dbrs")
                        || metadata.target().starts_with("workspace")
                        || metadata.target().starts_with("lb_fs")
                }))
                .boxed(),
        );

        // stdout target for non-android platforms
        #[cfg(not(target_os = "android"))]
        layers.push(
            fmt::Layer::new()
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                .pretty()
                .with_target(false)
                .with_filter(lockbook_log_level)
                .with_filter(filter::filter_fn(|metadata| {
                    metadata.target().starts_with("lb_rs")
                        || metadata.target().starts_with("dbrs")
                        || metadata.target().starts_with("workspace")
                        || metadata.target().starts_with("lb_fs")
                }))
                .boxed(),
        );

        // logcat target for android
        #[cfg(target_os = "android")]
        if let Some(writer) =
            tracing_logcat::LogcatMakeWriter::new(tracing_logcat::LogcatTag::Target).ok()
        {
            layers.push(
                fmt::Layer::new()
                    .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                    .with_ansi(false)
                    .with_writer(writer)
                    .with_filter(lockbook_log_level)
                    .with_filter(filter::filter_fn(|metadata| {
                        metadata.target().starts_with("lb_rs")
                            || metadata.target().starts_with("workspace")
                            || metadata.target().starts_with("lb_java")
                    }))
                    .boxed(),
            );
        }

        tracing::subscriber::set_global_default(
            tracing_subscriber::Registry::default().with(layers),
        )
        .map_err(core_err_unexpected)?;
        panic_capture(config);
    }
    Ok(())
}

fn panic_capture(config: &Config) {
    let path = config.writeable_path.clone();
    panic::set_hook(Box::new(move |panic_info| {
        let bt = Backtrace::force_capture();
        tracing::error!("panic detected: {panic_info} {}", bt);
        eprintln!("panic detected and logged: {panic_info} {}", bt);
        let timestamp = Local::now().format("%Y-%m-%d---%H-%M-%S");
        let file_name = format!("{}/panic---{}.log", path, timestamp);
        let file = format!("INFO: {}\nBT: {}", panic_info, bt);
        fs::write(file_name, file).unwrap();
    }));
}
