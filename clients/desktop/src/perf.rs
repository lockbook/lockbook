use lb::service::logging;

/// `process::exit` skips Drop but still runs atexit. Without this, quitting
/// loses the per-thread trace queues that have not filled yet.
pub fn install_exit_flush() {
    if !logging::traces_enabled() {
        return;
    }
    register_atexit();
    register_term_exit();
}

#[cfg(unix)]
fn register_atexit() {
    extern "C" fn flush_atexit() {
        logging::flush_traces();
    }
    // SAFETY: handler only flushes once at process exit.
    unsafe {
        let _ = libc::atexit(flush_atexit);
    }
}

#[cfg(unix)]
fn register_term_exit() {
    extern "C" fn on_term(_: libc::c_int) {
        // Not async-signal-safe; dogfood only. Routes SIGTERM through
        // `exit` so atexit can flush the trace queues.
        std::process::exit(0);
    }
    // SAFETY: replaces default terminate with exit+flush for this process.
    unsafe {
        libc::signal(libc::SIGTERM, on_term as libc::sighandler_t);
        libc::signal(libc::SIGINT, on_term as libc::sighandler_t);
    }
}

#[cfg(not(unix))]
fn register_atexit() {}

#[cfg(not(unix))]
fn register_term_exit() {}
