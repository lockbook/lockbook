use crate::config::Config;
use crate::CARGO_PKG_VERSION;
use futures::StreamExt;
use pagerduty_rs::eventsv2async::EventsV2;
use pagerduty_rs::types::{AlertTrigger, AlertTriggerPayload};
use std::any::TypeId;
use std::env;
use std::num::NonZeroU64;
use std::path::Path;
use std::time::SystemTime;
use tracing::metadata::LevelFilter;
use tracing::span::{Attributes, Record};
use tracing::subscriber::Interest;
use tracing::{Event, Id, Metadata, Subscriber};
use tracing_appender::rolling::RollingFileAppender;
use tracing_subscriber::filter::{FilterExt, FilterFn, Filtered};
use tracing_subscriber::fmt::format::{FmtSpan, Format};
use tracing_subscriber::layer::{Context, Filter, Layered};
use tracing_subscriber::{filter, fmt, prelude::*, Layer};

static LOG_FILE: &str = "lockbook_server.log";

pub fn init(config: &Config) {
    let log_path = Path::new(&config.server.log_path);

    let subscriber = tracing_subscriber::Registry::default()
        // Logger for stdout (local development)
        .with(
            fmt::Layer::new()
                .pretty()
                .with_filter(LevelFilter::INFO)
                .with_filter(server_logs()),
        )
        // Logger for file (verbose and sent to loki)
        .with(
            fmt::Layer::new()
                .with_writer(file_logger(config))
                .with_ansi(false)
                .with_filter(LevelFilter::DEBUG)
                .with_filter(server_logs()),
        )
        // Logger for disaster response (any error logs sent to pagerduty)
        .with(
            PDLogger { key: "".to_string(), config: config.clone() }
                .with_filter(LevelFilter::ERROR)
                .with_filter(server_logs()),
        );

    tracing::subscriber::set_global_default(subscriber).unwrap();
}

impl<S: Subscriber> Layer<S> for PDLogger {
    fn on_event(&self, _event: &Event<'_>, _ctx: Context<'_, S>) {
        println!("{:?}", _event.record())
    }

    fn on_record(&self, _span: &Id, _values: &Record<'_>, _ctx: Context<'_, S>) {
        println!("{:?}", _values)
    }
}

fn file_logger(config: &Config) -> RollingFileAppender {
    tracing_appender::rolling::never(&config.server.log_path, LOG_FILE)
}

fn server_logs() -> FilterFn {
    filter::filter_fn(|metadata| metadata.target().starts_with("lockbook_server"))
}

fn route_to_pagerduty(config: &Config) -> FilterFn {
    filter::filter_fn(|metadata| {
        println!("");
        false
    })
}

struct PDLogger {
    key: String,
    config: Config,
}

// fn pd_logger(config: &Config, handle: Handle) -> Dispatch {
//     let key = config.clone().server.pd_api_key.unwrap();
//     let config = config.clone();
//     notify(
//         &key,
//         &handle,
//         Event::Change(Change {
//             payload: ChangePayload {
//                 summary: String::from("Lockbook Server is starting up..."),
//                 timestamp: SystemTime::now().into(),
//                 source: Some(config.server.env.to_string()),
//                 custom_details: Some(ChangeDetail { build: CARGO_PKG_VERSION.to_string() }),
//             },
//             links: None,
//         }),
//     );
//
//     let pdl = PDLogger { key, handle, config };
// }
//
// struct PDLogger {
//     key: String,
//     handle: Handle,
//     config: Config,
// }
//
// fn dedup_key(record: &Record) -> String {
//     let mut hasher = Sha256::new();
//     hasher.update(record.args().to_string());
//     let result = hasher.finalize();
//     base64::encode(result)
// }
//
// impl Log for PDLogger {
//     /// This is where you decide what gets sent to pagerduty
//     /// Currently only errors are sent, and rustls::session is explicitly black-holed
//     /// Logs from rustls::session will still be logged to the file and standard streams, but they're
//     /// incredibly noisy and clients can cause them to "fatal log" quite easily (ex: attempt to connect via TLS 1.0)
//     fn enabled(&self, metadata: &Metadata) -> bool {
//         metadata.level() == Level::Error && metadata.target() != "rustls::session"
//     }
//
//     fn log(&self, record: &Record) {
//         if self.enabled(record.metadata()) {
//             notify(
//                 &self.key,
//                 &self.handle,
//                 Event::AlertTrigger(AlertTrigger {
//                     payload: AlertTriggerPayload {
//                         severity: level_to_severity(record.level()),
//                         summary: record.args().to_string(),
//                         source: self.config.server.env.to_string(),
//                         timestamp: Some(SystemTime::now().into()),
//                         component: None,
//                         group: None,
//                         class: None,
//                         custom_details: Some(LogDetails {
//                             data: record.args().to_string(),
//                             logger: record.target().to_string(),
//                             file: record.file().map(|c| c.to_string()),
//                             line: record.line().map(|c| c.to_string()),
//                             build: CARGO_PKG_VERSION.to_string(),
//                         }),
//                     },
//                     dedup_key: Some(dedup_key(record)),
//                     images: None,
//                     links: None,
//                     client: None,
//                     client_url: None,
//                 }),
//             );
//         }
//     }
//
//     fn flush(&self) {}
// }
//
// fn level_to_severity(level: Level) -> Severity {
//     match level {
//         Level::Error => Severity::Error,
//         Level::Warn => Severity::Info,
//         Level::Info => Severity::Info,
//         Level::Debug => Severity::Info,
//         Level::Trace => Severity::Info,
//     }
// }
//
// #[derive(Serialize)]
// struct LogDetails<T: serde::Serialize> {
//     data: T,
//     logger: String,
//     file: Option<String>,
//     line: Option<String>,
//     build: String,
// }
//
// #[derive(Serialize)]
// struct ChangeDetail {
//     build: String,
// }
//
// fn notify<T: serde::Serialize + std::marker::Send + std::marker::Sync + 'static>(
//     api_key: &str, handle: &Handle, event: Event<T>,
// ) {
//     let events = EventsV2::new(String::from(api_key), Some("lockbook-server".to_string())).unwrap();
//
//     // https://github.com/neonphog/tokio_safe_block_on/blob/074d40929ccab649b0dcc83a4ebdbdcb70b317fb/src/lib.rs#L72-L86
//     tokio::task::block_in_place(move || {
//         futures::executor::block_on(async {
//             handle
//                 .spawn(async move {
//                     events
//                         .event(event)
//                         .await
//                         .err()
//                         .map(|err| eprintln!("Failed reporting event to PagerDuty! {}", err))
//                 })
//                 .await
//                 .err()
//                 .map(|err| eprintln!("Failed spawning task in Tokio runtime! {}", err))
//         })
//     });
// }
