use fern::colors::{Color, ColoredLevelConfig};
use fern::Dispatch;
use log::{Level, Log, Metadata, Record};
use pagerduty_rs::eventsv2async::EventsV2;
use pagerduty_rs::types::{Change, ChangePayload, Event};
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
        &"PagerDuty client has connected, server is hot.".to_string(),
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
        metadata.level() <= Level::Warn
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            notify(&self.key, &self.handle, &record.args().to_string());
        }
    }

    fn flush(&self) {}
}

fn notify(api_key: &String, handle: &Handle, message: &String) {
    let events = EventsV2::new(api_key.to_string(), Some("lockbook-server".to_string())).unwrap();
    let e = Event::Change(Change {
        payload: ChangePayload {
            summary: message.to_string(),
            timestamp: SystemTime::now().into(),
            source: Some("lockbook-server".to_string()),
            custom_details: Option::<()>::None,
        },
        links: None,
    });

    futures::executor::block_on(async {
        handle
            .spawn(async move { events.event(e).await })
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
