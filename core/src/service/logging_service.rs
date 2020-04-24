use crate::service::logging_service::LogLevel::{All, Error, Info, Silent, Warn};
use termion::color;

pub trait Logger {
    fn debug(msg: String);
    fn info(msg: String);
    fn warn(msg: String);
    fn error(msg: String);
}

pub struct VerboseStdOut;

impl Logger for VerboseStdOut {
    fn debug(msg: String) {
        println!(
            "{}{}{}",
            color::Fg(color::Yellow),
            msg,
            color::Fg(color::Reset)
        )
    }
    fn info(msg: String) {
        println!(
            "{}{}{}",
            color::Fg(color::Cyan),
            msg,
            color::Fg(color::Reset)
        )
    }
    fn warn(msg: String) {
        println!(
            "{}{}{}",
            color::Fg(color::Magenta),
            msg,
            color::Fg(color::Reset)
        )
    }
    fn error(msg: String) {
        eprintln!(
            "{}{}{}",
            color::Fg(color::Red),
            msg,
            color::Fg(color::Reset)
        )
    }
}

pub struct ConditionalStdOut;

fn get_log_level() -> LogLevel {
    match std::env::var("LOCKBOOK_LOG_LEVEL") {
        Ok(value) => match value.to_lowercase().as_str() {
            "all" | "verbose" | "debug" => All,
            "info" => Info,
            "warn" => Warn,
            "error" => Error,
            "silent" => Silent,
            _ => All,
        },
        Err(_) => Error,
    }
}

enum LogLevel {
    All,
    Info,
    Warn,
    Error,
    Silent,
}

impl Logger for ConditionalStdOut {
    fn debug(msg: String) {
        match get_log_level() {
            All => println!(
                "{}{}{}",
                color::Fg(color::Yellow),
                msg,
                color::Fg(color::Reset)
            ),
            _ => {}
        }
    }
    fn info(msg: String) {
        match get_log_level() {
            All | Info => println!(
                "{}{}{}",
                color::Fg(color::Cyan),
                msg,
                color::Fg(color::Reset)
            ),
            _ => {}
        }
    }
    fn warn(msg: String) {
        match get_log_level() {
            All | Info | Warn => println!(
                "{}{}{}",
                color::Fg(color::Magenta),
                msg,
                color::Fg(color::Reset)
            ),
            _ => {}
        }
    }
    fn error(msg: String) {
        match get_log_level() {
            All | Info | Warn | Error => eprintln!(
                "{}{}{}",
                color::Fg(color::Red),
                msg,
                color::Fg(color::Reset)
            ),
            _ => {}
        }
    }
}

pub struct BlackHole;

impl Logger for BlackHole {
    fn debug(_msg: String) {}
    fn info(_msg: String) {}
    fn warn(_msg: String) {}
    fn error(_msg: String) {}
}
