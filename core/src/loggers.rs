use fern::colors::{Color, ColoredLevelConfig};
use fern::Dispatch;
use std::path::Path;
use std::{fs, io};

pub fn init(log_path: &Path, log_name: String, std_colors: bool) -> Result<Dispatch, io::Error> {
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
        .chain(std::io::stdout());

    let _ = fs::create_dir_all(log_path)?;
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

    Ok(fern::Dispatch::new()
        .chain(stdout_logger)
        .chain(file_logger))
}
