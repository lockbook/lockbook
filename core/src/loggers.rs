use crate::model::state::Config;
use crate::LOG_FILE;
use fern::colors::{Color, ColoredLevelConfig};
use std::io;

#[derive(Debug)]
pub enum LoggersError {
    File(io::Error),
    Set(log::SetLoggerError),
}

pub fn init(config: &Config, std_debug: bool, std_colors: bool) -> Result<(), LoggersError> {
    let colors_level = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Green)
        .debug(Color::Blue)
        .trace(Color::Black);

    let stdout_lb_level = if std_debug {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

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
                    "[{timestamp}] [{target:<40}] [{level:<5}]: {message}\x1B[0m",
                    timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                    target = record.target(),
                    level = record.level(),
                    message = message.clone(),
                ))
            }
        })
        .chain(std::io::stdout())
        .level(log::LevelFilter::Off)
        .level_for("lockbook_core", stdout_lb_level);

    let log_file = fern::log_file(config.path().join(LOG_FILE)).map_err(LoggersError::File)?;

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
        .chain(log_file)
        .level(log::LevelFilter::Debug);

    fern::Dispatch::new()
        .chain(stdout_logger)
        .chain(file_logger)
        .apply()
        .map_err(LoggersError::Set)
}
