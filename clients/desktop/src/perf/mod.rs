//! Live performance tracing for engineer dogfood.
//!
//! Enable with:
//! `cargo run -p lockbook-desktop --features perf-qa`
//!
//! Writes `{writeable_path}/traces/<timestamp>.pftrace` (next to `lockbook.log`).
//! Open in https://ui.perfetto.dev.

#[cfg(feature = "perf-qa")]
mod metrics;

#[cfg(feature = "perf-qa")]
pub use metrics::Sample;

/// No-op when the Perfetto sink is not compiled in.
#[cfg(not(feature = "perf-qa"))]
pub struct Sample;

#[cfg(not(feature = "perf-qa"))]
impl Sample {
    pub fn new(_: &'static str) -> Option<Self> {
        None
    }
}

/// Install process logging. With `perf-qa`, also a Perfetto file.
/// Keep the returned session alive for the process lifetime.
pub fn install() -> Option<TraceSession> {
    let config = lb::model::core_config::Config::ui_config("egui");

    #[cfg(feature = "perf-qa")]
    {
        match install_perfetto(&config) {
            Ok(session) => Some(session),
            Err(e) => {
                eprintln!("perf-qa: failed to start Perfetto: {e}");
                lb::service::logging::install_default(&config).expect("install lockbook logging");
                None
            }
        }
    }

    #[cfg(not(feature = "perf-qa"))]
    {
        lb::service::logging::install_default(&config).expect("install lockbook logging");
        None
    }
}

pub struct TraceSession {
    #[cfg(feature = "perf-qa")]
    guard: std::sync::Arc<tracing_perfetto_file::FlushGuard>,
}

#[cfg(feature = "perf-qa")]
impl Drop for TraceSession {
    fn drop(&mut self) {
        let _ = self.guard.flush();
    }
}

#[cfg(feature = "perf-qa")]
fn install_perfetto(config: &lb::model::core_config::Config) -> std::io::Result<TraceSession> {
    use std::fs::{self, File};
    use std::sync::Arc;

    use lb::service::logging;
    use tracing::metadata::LevelFilter;
    use tracing_perfetto_file::{PerfettoLayer, SpanMode};
    use tracing_subscriber::Layer;
    use tracing_subscriber::Registry;
    use tracing_subscriber::filter;
    use tracing_subscriber::layer::SubscriberExt;

    fs::create_dir_all(&config.writeable_path)?;
    let traces = std::path::Path::new(&config.writeable_path).join("traces");
    fs::create_dir_all(&traces)?;

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let path = traces.join(format!("{stamp}.pftrace"));
    let file = File::create(&path)?;

    let (layer, guard) = PerfettoLayer::builder(file)
        .span_mode(SpanMode::Both)
        .with_debug_annotations()
        .with_source_locations()
        .with_counters()
        .build();
    let guard = Arc::new(guard);

    let perfetto = layer
        .with_filter(LevelFilter::TRACE)
        .with_filter(filter::filter_fn(|meta| {
            let t = meta.target();
            t.starts_with("lockbook_desktop")
                || t.starts_with("workspace")
                || t.starts_with("lb_rs")
                || t.starts_with("dbrs")
                || t.starts_with("lb_fs")
        }));

    let subscriber = Registry::default()
        .with(logging::fmt_layers(config))
        .with(perfetto);

    tracing::subscriber::set_global_default(subscriber)
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    let flush = Arc::clone(&guard);
    logging::panic_capture(
        config,
        Some(Box::new(move || {
            let _ = flush.flush();
        })),
    );

    Ok(TraceSession { guard })
}

#[cfg(all(test, feature = "perf-qa"))]
mod tests {
    use std::fs::File;

    use tracing_perfetto_file::PerfettoLayer;
    use tracing_subscriber::Registry;
    use tracing_subscriber::layer::SubscriberExt;

    #[test]
    fn perfetto_layer_writes_a_file() {
        let path = std::env::temp_dir().join("lockbook-perf-qa-smoke.pftrace");
        let file = File::create(&path).expect("create smoke trace");
        let (layer, guard) = PerfettoLayer::builder(file)
            .with_debug_annotations()
            .with_counters()
            .build();
        let subscriber = Registry::default().with(layer);
        tracing::subscriber::with_default(subscriber, || {
            let _span = tracing::info_span!("apply", action = "Create").entered();
            tracing::trace!(kind = "apply", wall_us = 12u64, counter.rss_bytes = 1u64, "sample");
        });
        guard.flush().expect("flush");
        let len = std::fs::metadata(&path).expect("stat").len();
        assert!(len > 0, "expected a non-empty .pftrace");
        let _ = std::fs::remove_file(path);
    }
}
