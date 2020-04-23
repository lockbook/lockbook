use crate::service::logging_service::LogLevel::{All, Debug, Error, Silent, Warn};

pub trait Logger {
    fn info(msg: String);
    fn debug(msg: String);
    fn warn(msg: String);
    fn error(msg: String);
}

pub struct VerboseStdOut;

impl Logger for VerboseStdOut {
    fn info(msg: String) {
        println!("ℹ️ {}", msg)
    }
    fn debug(msg: String) {
        println!("🚧 {}", msg)
    }
    fn warn(msg: String) {
        println!("⚠️ {}", msg)
    }
    fn error(msg: String) {
        eprintln!("🛑 {}", msg)
    }
}

pub struct ConditionalStdOut;

fn get_log_level() -> LogLevel {
    match std::env::var("LOCKBOOK_LOG_LEVEL") {
        Ok(value) => match value.to_lowercase().as_str() {
            "all" => All,
            "debug" => Debug,
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
    Debug,
    Warn,
    Error,
    Silent,
}

impl Logger for ConditionalStdOut {
    fn info(msg: String) {
        match get_log_level() {
            All => println!("ℹ️ {}", msg),
            _ => {}
        }
    }
    fn debug(msg: String) {
        match get_log_level() {
            All | Debug => println!("ℹ️ {}", msg),
            _ => {}
        }
    }
    fn warn(msg: String) {
        match get_log_level() {
            All | Debug | Warn => println!("ℹ️ {}", msg),
            _ => {}
        }
    }
    fn error(msg: String) {
        match get_log_level() {
            All | Debug | Warn | Error => println!("ℹ️ {}", msg),
            _ => {}
        }
    }
}

pub struct BlackHole;

impl Logger for BlackHole {
    fn info(_msg: String) {}
    fn debug(_msg: String) {}
    fn warn(_msg: String) {}
    fn error(_msg: String) {}
}
