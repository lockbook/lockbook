use crate::Config;
use crate::model::errors::{LbResult, core_err_unexpected};
use crate::service::debug::{generate_panic_content, generate_panic_filename};
use std::backtrace::Backtrace;
use std::fs::OpenOptions;
use std::io::Write;
use std::{env, panic};
use tracing::Subscriber;
use tracing::metadata::LevelFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::prelude::*;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::{Layer, filter, fmt};

pub static LOG_FILE: &str = "lockbook.log";

fn log_targets(metadata: &tracing::Metadata<'_>) -> bool {
    let t = metadata.target();
    t.starts_with("lb_rs")
        || t.starts_with("dbrs")
        || t.starts_with("workspace")
        || t.starts_with("lb_fs")
}

/// File + stdout/logcat layers. Compose with extra layers, then
/// `set_global_default`, or call [`install_default`].
pub fn fmt_layers<S>(config: &Config) -> Vec<Box<dyn Layer<S> + Send + Sync + 'static>>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    let lockbook_log_level = env::var("LOG_LEVEL")
        .ok()
        .and_then(|s| s.as_str().parse().ok())
        .unwrap_or(LevelFilter::DEBUG);

    let mut layers = Vec::with_capacity(2);

    #[cfg(not(target_arch = "wasm32"))]
    layers.push(
        fmt::Layer::new()
            .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
            .with_ansi(config.colored_logs)
            .with_target(true)
            .with_writer(tracing_appender::rolling::never(&config.writeable_path, LOG_FILE))
            .with_filter(lockbook_log_level)
            .with_filter(filter::filter_fn(log_targets))
            .boxed(),
    );

    if config.stdout_logs {
        #[cfg(not(target_os = "android"))]
        layers.push(
            fmt::Layer::new()
                .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
                .with_ansi(config.colored_logs)
                .with_target(true)
                .with_filter(lockbook_log_level)
                .with_filter(filter::filter_fn(log_targets))
                .boxed(),
        );
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
                        let t = metadata.target();
                        t.starts_with("lb_rs")
                            || t.starts_with("workspace")
                            || t.starts_with("lb_java")
                    }))
                    .boxed(),
            );
        }
    }

    layers
}

/// Install the default `lockbook.log` subscriber. Call from process entry
/// before [`crate::Lb::init`]. No-op if `config.logs` is false or a subscriber
/// is already set.
pub fn install_default(config: &Config) -> LbResult<()> {
    if !config.logs {
        return Ok(());
    }
    if tracing::dispatcher::has_been_set() {
        return Ok(());
    }
    if !config.writeable_path.is_empty() {
        std::fs::create_dir_all(&config.writeable_path).map_err(core_err_unexpected)?;
    }
    tracing::subscriber::set_global_default(
        tracing_subscriber::Registry::default().with(fmt_layers(config)),
    )
    .map_err(core_err_unexpected)?;
    panic_capture(config, None);
    Ok(())
}

/// Panic file next to the log. `extra` runs after the file write (e.g. flush a
/// trace). Replaces any previous hook.
pub fn panic_capture(config: &Config, extra: Option<Box<dyn Fn() + Send + Sync>>) {
    let path = config.writeable_path.clone();
    panic::set_hook(Box::new(move |error_header| {
        let bt = Backtrace::force_capture();
        tracing::error!("panic detected: {error_header} {}", bt);
        eprintln!("panic detected and logged: {error_header} {bt}");
        let file_name = generate_panic_filename(&path);
        let content = generate_panic_content(&error_header.to_string(), &bt.to_string());

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_name)
            .unwrap();

        file.write_all(content.as_bytes()).unwrap();

        if let Some(extra) = extra.as_ref() {
            extra();
        }
    }));
}
