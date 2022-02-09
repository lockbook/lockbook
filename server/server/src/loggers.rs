use crate::config::Config;
use crate::CARGO_PKG_VERSION;
use fern::colors::{Color, ColoredLevelConfig};
use fern::Dispatch;
use log::{Level, Log, Metadata, Record};
use pagerduty_rs::eventsv2async::EventsV2;
use pagerduty_rs::types::{
    AlertTrigger, AlertTriggerPayload, Change, ChangePayload, Event, Severity,
};
use serde::Serialize;
use std::fs;
use std::path::Path;
use std::time::SystemTime;
use tokio::runtime::Handle;

static LOG_FILE: &str = "lockbook_server.log";

pub fn init(config: &Config) {
    let log_path = Path::new(&config.server.log_path);
    let handle = Handle::current();
    let std_colors = true;

    let colors_level = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Green)
        .debug(Color::Blue)
        .trace(Color::Black);

    let stdout_logger = fern::Dispatch::new()
        .format(move |out, message, record| {
            if std_colors {
                out.finish(format_args!(
                    "[{timestamp}] [{target:<40}] [{level:<5}]: {message}\x1B[0m",
                    timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    target = record.target(),
                    level = colors_level.color(record.level()),
                    message = message.clone(),
                ))
            } else {
                out.finish(format_args!(
                    "[{timestamp}] [{target:<40}] [{level:<5}]: {message}",
                    timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    target = record.target(),
                    level = record.level(),
                    message = message.clone(),
                ))
            }
        })
        .chain(std::io::stdout())
        .level(log::LevelFilter::Warn);

    fs::create_dir_all(log_path).expect("unable to create directory for logger");
    let log_file = fern::log_file(log_path.join(LOG_FILE)).expect("unable to create log file");

    let file_logger = fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{timestamp}] [{target:<40}] [{level:<5}]: {message}\x1B[0m",
                timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                target = record.target(),
                level = record.level(),
                message = message.clone(),
            ))
        })
        .chain(log_file);

    let base_logger = fern::Dispatch::new()
        .chain(stdout_logger)
        .chain(file_logger);

    match config.server.pd_api_key.as_ref() {
        None => base_logger,
        Some(api_key) => base_logger.chain(pd_logger(CARGO_PKG_VERSION, api_key, handle)),
    }
    .level(log::LevelFilter::Info)
    .level_for("lockbook_server", log::LevelFilter::Debug)
    .apply()
    .expect("Failed setting logger!");
}

fn pd_logger(build: &str, pd_api_key: &str, handle: Handle) -> Dispatch {
    notify(
        pd_api_key,
        &handle,
        Event::Change(Change {
            payload: ChangePayload {
                summary: String::from("Lockbook Server is starting up..."),
                timestamp: SystemTime::now().into(),
                source: Some(String::from("localhost")), // TODO: Hostname
                custom_details: Some(ChangeDetail { build: String::from(build) }),
            },
            links: None,
        }),
    );

    let pdl = PDLogger { key: String::from(pd_api_key), handle, build: String::from(build) };

    fern::Dispatch::new()
        .format(move |out, message, _| {
            out.finish(format_args!("{message}", message = message.clone()))
        })
        .chain(Box::new(pdl) as Box<dyn Log>)
}

struct PDLogger {
    key: String,
    handle: Handle,
    build: String,
}

impl Log for PDLogger {
    /// This is where you decide what getss sent to pagerduty
    /// Currently only errors are sent, and rustls::session is explicitly black-holed
    /// Logs from rustls::session will still be logged to the file and standard streams, but they're
    /// incredibly noisy and clients can cause them to "fatal log" quite easily (ex: attempt to connect via TLS 1.0)
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() == Level::Error && metadata.target() != "rustls::session"
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            notify(
                &self.key,
                &self.handle,
                Event::AlertTrigger(AlertTrigger {
                    payload: AlertTriggerPayload {
                        severity: level_to_severity(record.level()),
                        summary: record.args().to_string(),
                        source: "localhost".to_string(), // TODO: Hostname
                        timestamp: Some(SystemTime::now().into()),
                        component: None,
                        group: None,
                        class: None,
                        custom_details: Some(LogDetails {
                            data: record.args().to_string(),
                            logger: record.target().to_string(),
                            file: record.file().map(|c| c.to_string()),
                            line: record.line().map(|c| c.to_string()),
                            build: self.build.to_string(),
                        }),
                    },
                    dedup_key: None,
                    images: None,
                    links: None,
                    client: None,
                    client_url: None,
                }),
            );
        }
    }

    fn flush(&self) {}
}

fn level_to_severity(level: Level) -> Severity {
    match level {
        Level::Error => Severity::Error,
        Level::Warn => Severity::Info,
        Level::Info => Severity::Info,
        Level::Debug => Severity::Info,
        Level::Trace => Severity::Info,
    }
}

#[derive(Serialize)]
struct LogDetails<T: serde::Serialize> {
    data: T,
    logger: String,
    file: Option<String>,
    line: Option<String>,
    build: String,
}

#[derive(Serialize)]
struct ChangeDetail {
    build: String,
}

fn notify<T: serde::Serialize + std::marker::Send + std::marker::Sync + 'static>(
    api_key: &str, handle: &Handle, event: Event<T>,
) {
    let events = EventsV2::new(String::from(api_key), Some("lockbook-server".to_string())).unwrap();

    // https://github.com/neonphog/tokio_safe_block_on/blob/074d40929ccab649b0dcc83a4ebdbdcb70b317fb/src/lib.rs#L72-L86
    tokio::task::block_in_place(move || {
        futures::executor::block_on(async {
            handle
                .spawn(async move {
                    events
                        .event(event)
                        .await
                        .err()
                        .map(|err| eprintln!("Failed reporting event to PagerDuty! {}", err))
                })
                .await
                .err()
                .map(|err| eprintln!("Failed spawning task in Tokio runtime! {}", err))
        })
    });
}
