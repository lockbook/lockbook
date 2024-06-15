use crate::model::errors::core_err_unexpected;
use crate::{Config, LbResult};
use signal_hook::{consts::signal::*, iterator::Signals};
use std::{backtrace, env, panic};
use std::{process, thread};
use tracing::metadata::LevelFilter;
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::{filter, fmt, prelude::*, Layer};

static LOG_FILE: &str = "lockbook.log";

pub fn init(config: &Config) -> LbResult<()> {
    if config.logs {
        let lockbook_log_level = env::var("LOG_LEVEL")
            .ok()
            .and_then(|s| s.as_str().parse().ok())
            .unwrap_or(LevelFilter::DEBUG);

        let subscriber =
            tracing_subscriber::Registry::default()
                .with(
                    fmt::Layer::new()
                        .with_span_events(FmtSpan::ACTIVE)
                        .with_ansi(config.colored_logs)
                        .with_target(true)
                        .with_writer(tracing_appender::rolling::never(
                            &config.writeable_path,
                            LOG_FILE,
                        ))
                        .with_filter(lockbook_log_level)
                        .with_filter(filter::filter_fn(|metadata| {
                            metadata.target().starts_with("lb_rs")
                                || metadata.target().starts_with("dbrs")
                        })),
                )
                .with(fmt::Layer::new().pretty().with_target(false).with_filter(
                    filter::filter_fn(|metadata| metadata.target().starts_with("lb_fs")),
                ));

        tracing::subscriber::set_global_default(subscriber).map_err(core_err_unexpected)?;
        panic_capture();
        signal_capture();
    }
    Ok(())
}

fn panic_capture() {
    panic::set_hook(Box::new(|panic_info| {
        tracing::error!("panic detected: {panic_info} {}", backtrace::Backtrace::force_capture());
    }));
}

fn signal_capture() {
    // thank you chatgpt, I checked like 2 of them and found them to be correct
    match Signals::new([
        SIGHUP,  // SIGHUP (1): Hangup detected on controlling terminal or death of controlling process
        SIGINT,  // SIGINT (2): Interrupt from keyboard (Ctrl+C)
        SIGQUIT, // SIGQUIT (3): Quit from keyboard (Ctrl+\)
        SIGILL,  // SIGILL (4): Illegal instruction
        SIGABRT, // SIGABRT (6): Abort signal from abort(3)
        SIGFPE,  // SIGFPE (8): Floating-point exception
        SIGKILL, // SIGKILL (9): Kill signal (cannot be caught, blocked, or ignored)
        SIGSEGV, // SIGSEGV (11): Invalid memory reference
        SIGPIPE, // SIGPIPE (13): Broken pipe (write to a pipe with no readers)
        SIGALRM, // SIGALRM (14): Timer signal from alarm(2)
        SIGTERM, // SIGTERM (15): Termination signal
        SIGUSR1, // SIGUSR1 (10): User-defined signal 1
        SIGUSR2, // SIGUSR2 (12): User-defined signal 2
        SIGXCPU, // SIGXCPU (24): CPU time limit exceeded
        SIGXFSZ, // SIGXFSZ (25): File size limit exceeded
        SIGSYS,  // SIGSYS (31 on Linux, 12 on macOS): Bad argument to routine (SVr4)
    ]) {
        Ok(mut signals) => {
            debug!("successfully listening for signals");

            thread::spawn(move || {
                if let Some(signal) = signals.forever().next() {
                    tracing::error!("Terminal signal recieved: {signal}");
                    // apparently idiomatic
                    process::exit(signal + 128);
                }
            });
        }
        Err(e) => {
            tracing::error!("unable to hookup signal handler: {e}");
        }
    }
}
