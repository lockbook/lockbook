use crate::model::errors::core_err_unexpected;
use crate::{Config, CoreError};
use std::any::TypeId;
use std::env;
use std::path::Path;
use tracing::field::Field;
use tracing::metadata::LevelFilter;
use tracing::span::{Attributes, Record};
use tracing::subscriber::Interest;
use tracing::{Event, Id, Level, Metadata, Subscriber};
use tracing_subscriber::field::Visit;
use tracing_subscriber::filter::Filtered;
use tracing_subscriber::fmt::format::{FmtSpan, Format};
use tracing_subscriber::layer::{Context, Filter, Layered};
use tracing_subscriber::{filter, fmt, prelude::*, Layer};

static LOG_FILE: &str = "lockbook.log";

pub fn init(config: &Config) -> Result<(), CoreError> {
    if config.logs {
        let lockbook_log_level = env::var("LOG_LEVEL")
            .ok()
            .and_then(|s| s.as_str().parse().ok())
            .unwrap_or(LevelFilter::DEBUG);

        let subscriber = tracing_subscriber::Registry::default().with(
            Parth {}
                // .with_span_events(FmtSpan::ACTIVE)
                // .with_ansi(config.colored_logs)
                // .with_target(false)
                // .with_writer(tracing_appender::rolling::never(&config.writeable_path, LOG_FILE))
                .with_filter(lockbook_log_level)
                .with_filter(filter::filter_fn(|metadata| {
                    metadata.target().starts_with("lockbook_core")
                })),
        );

        tracing::subscriber::set_global_default(subscriber).map_err(core_err_unexpected)?;
        let test = span!(Level::INFO, "test_span", b = 6);
        let guard = test.enter();
        event!(Level::INFO, a = 5, "test");
    }
    Ok(())
}
use core::fmt::Debug;
struct Parth {}
impl Visit for Parth {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        println!("{} = {:?}; ", field.name(), value);
    }
}
impl<S: Subscriber> Layer<S> for Parth {
    fn on_event(&self, _event: &Event<'_>, _ctx: Context<'_, S>) {
        println!("{:?}", _event.record(&mut Parth {}))
    }
    fn on_record(&self, _span: &Id, _values: &Record<'_>, _ctx: Context<'_, S>) {
        println!("{:?}", _values)
    }
}
