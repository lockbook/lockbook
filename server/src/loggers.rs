use fern::colors::{Color, ColoredLevelConfig};
use fern::Dispatch;
use log::{Level, Log, Metadata, Record};
use pagerduty_rs::eventsv2async::EventsV2;
use pagerduty_rs::types::{
    AlertTrigger, AlertTriggerPayload, Change, ChangePayload, Event, Severity,
};
use serde::Serialize;
use std::path::Path;
use std::time::SystemTime;
use std::{fs, io};
use tokio::runtime::Handle;

pub fn init(
    log_path: &Path,
    log_name: String,
    std_colors: bool,
    pd_api_key: &Option<String>,
    handle: Handle,
) -> Result<Dispatch, io::Error> {
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

    fs::create_dir_all(log_path)?;
    let log_file = fern::log_file(log_path.join(log_name))?;

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

    match pd_api_key {
        None => Ok(base_logger),
        Some(api_key) => {
            let pd_logger = fern::Dispatch::new()
                .format(move |out, message, _| {
                    out.finish(format_args!("{message}", message = message.clone()))
                })
                .chain(pd_logger(api_key, handle));
            Ok(base_logger.chain(pd_logger))
        }
    }
}

fn pd_logger(pd_api_key: &String, handle: Handle) -> Box<dyn Log> {
    let _ = notify(
        pd_api_key,
        &handle,
        Event::Change(Change {
            payload: ChangePayload {
                summary: "Lockbook Server is starting up...".to_string(),
                timestamp: SystemTime::now().into(),
                source: Some("localhost".to_string()), // TODO: Hostname
                custom_details: Option::<()>::None,
            },
            links: None,
        }),
    );

    let pdl = PDLogger {
        key: pd_api_key.to_string(),
        handle: handle,
    };

    Box::new(pdl)
}

struct PDLogger {
    key: String,
    handle: Handle,
}

impl Log for PDLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Error
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
                        }),
                    },
                    dedup_key: Some(dedup_key(record)),
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

fn dedup_key(record: &Record) -> String {
    match (record.file(), record.line()) {
        (Some(file), Some(line)) => {
            format!("{}#{}", file, line)
        }
        (_, _) => record
            .target()
            .chars()
            .take(60)
            .chain("::".chars())
            .chain(record.args().to_string().chars())
            .take(255)
            .collect(),
    }
}

#[derive(Serialize)]
struct LogDetails<T: serde::Serialize> {
    data: T,
    logger: String,
}

fn notify<T: serde::Serialize + std::marker::Send + std::marker::Sync + 'static>(
    api_key: &String,
    handle: &Handle,
    event: Event<T>,
) {
    let events = EventsV2::new(api_key.to_string(), Some("lockbook-server".to_string())).unwrap();

    futures::executor::block_on(async {
        handle
            .spawn(async move { events.event(event).await })
            .await
            .expect("Task spawned in Tokio executor panicked")
            .unwrap_or_else(|_| {})
    });
}

fn event_to_json<T: Serialize>(event: &Event<T>) -> String {
    match event {
        Event::Change(c) => serde_json::to_string(c).unwrap(),
        Event::AlertTrigger(at) => serde_json::to_string(at).unwrap(),
        Event::AlertAcknowledge(aa) => serde_json::to_string(aa).unwrap(),
        Event::AlertResolve(ar) => serde_json::to_string(ar).unwrap(),
    }
}
