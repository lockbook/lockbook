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

/// Set to `1` to write `{writeable_path}/traces/<timestamp>.pftrace`, or to a
/// path to write there. Open the result in <https://ui.perfetto.dev>.
pub static TRACE_ENV_VAR: &str = "LB_TRACE";

fn log_targets(metadata: &tracing::Metadata<'_>) -> bool {
    let t = metadata.target();
    t.starts_with("lb_rs")
        || t.starts_with("dbrs")
        || t.starts_with("workspace")
        || t.starts_with("lb_fs")
        || t.starts_with("lockbook_desktop")
}

#[cfg(not(target_family = "wasm"))]
fn trace_targets(metadata: &tracing::Metadata<'_>) -> bool {
    log_targets(metadata) || metadata.target().starts_with("lockbook_desktop")
}

fn fmt_layers<S>(config: &Config) -> Vec<Box<dyn Layer<S> + Send + Sync + 'static>>
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

#[cfg(not(target_family = "wasm"))]
static TRACE_GUARD: std::sync::OnceLock<tracing_perfetto_file::FlushGuard> =
    std::sync::OnceLock::new();

#[cfg(not(target_family = "wasm"))]
fn trace_path(config: &Config) -> Option<std::path::PathBuf> {
    let requested = env::var(TRACE_ENV_VAR).ok()?;
    match requested.as_str() {
        "" | "0" | "false" => None,
        "1" | "true" => {
            let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
            Some(
                std::path::Path::new(&config.writeable_path)
                    .join("traces")
                    .join(format!("{stamp}.pftrace")),
            )
        }
        path => Some(std::path::PathBuf::from(path)),
    }
}

#[cfg(not(target_family = "wasm"))]
fn trace_file(path: &std::path::Path) -> std::io::Result<std::fs::File> {
    if let Some(dir) = path.parent().filter(|dir| !dir.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::File::create(path)
}

#[cfg(not(target_family = "wasm"))]
fn trace_layer<S>(config: &Config) -> Option<Box<dyn Layer<S> + Send + Sync + 'static>>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
{
    use tracing_perfetto_file::{PerfettoLayer, SpanMode};

    let path = trace_path(config)?;

    let file = match trace_file(&path) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("{TRACE_ENV_VAR}: failed to open {}: {err}", path.display());
            return None;
        }
    };

    let (layer, guard) = PerfettoLayer::builder(file)
        .span_mode(SpanMode::Both)
        .with_debug_annotations()
        .with_source_locations()
        .with_counters()
        .build();
    let _ = TRACE_GUARD.set(guard);

    eprintln!("{TRACE_ENV_VAR}: writing {} (open in https://ui.perfetto.dev)", path.display());

    Some(
        layer
            .with_filter(LevelFilter::TRACE)
            .with_filter(filter::filter_fn(trace_targets))
            .boxed(),
    )
}

/// True once [`init`] has opened a trace file.
pub fn traces_enabled() -> bool {
    #[cfg(not(target_family = "wasm"))]
    {
        TRACE_GUARD.get().is_some()
    }
    #[cfg(target_family = "wasm")]
    {
        false
    }
}

/// Write buffered spans to the trace file. A no-op when tracing is off; safe to
/// call from a process exit hook.
pub fn flush_traces() {
    #[cfg(not(target_family = "wasm"))]
    if let Some(guard) = TRACE_GUARD.get() {
        let _ = guard.flush();
    }
}

/// Install the process subscriber: `lockbook.log`, stdout/logcat, and (with
/// [`TRACE_ENV_VAR`]) a Perfetto trace file. Called by `Lb::init`; call it
/// earlier from a process entry point to capture boot.
pub fn init(config: &Config) -> LbResult<()> {
    if !config.logs {
        return Ok(());
    }
    if tracing::dispatcher::has_been_set() {
        return Ok(());
    }
    if !config.writeable_path.is_empty() {
        std::fs::create_dir_all(&config.writeable_path).map_err(core_err_unexpected)?;
    }

    let subscriber = tracing_subscriber::Registry::default().with(fmt_layers(config));

    #[cfg(not(target_family = "wasm"))]
    let subscriber = subscriber.with(trace_layer(config));

    tracing::subscriber::set_global_default(subscriber).map_err(core_err_unexpected)?;
    panic_capture(config);
    Ok(())
}

fn panic_capture(config: &Config) {
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

        flush_traces();
    }));
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests {
    use super::*;
    use crate::model::core_config::ClientType;

    #[test]
    fn lb_trace_writes_a_pftrace() {
        let path = std::env::temp_dir().join("lockbook-lb-trace-smoke.pftrace");
        let _ = std::fs::remove_file(&path);
        env::set_var(TRACE_ENV_VAR, &path);

        let config = Config {
            writeable_path: String::new(),
            background_work: false,
            logs: true,
            stdout_logs: false,
            colored_logs: false,
            client_type: ClientType::Unknown,
        };
        let subscriber = tracing_subscriber::Registry::default().with(trace_layer(&config));
        env::remove_var(TRACE_ENV_VAR);

        tracing::subscriber::with_default(subscriber, || {
            let _span = tracing::trace_span!("smoke").entered();
            tracing::trace!(counter.rss_bytes = 1u64, "sample");
        });
        flush_traces();

        let len = std::fs::metadata(&path).expect("stat trace file").len();
        assert!(len > 0, "expected a non-empty .pftrace");
        let _ = std::fs::remove_file(&path);
    }
}
