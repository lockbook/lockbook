//! Spike: Lockbook as a Windows Cloud Files provider (placeholders in Explorer).

#[cfg(windows)]
mod cf;
#[cfg(windows)]
mod provider;
#[cfg(windows)]
mod watch;

#[cfg(windows)]
fn main() {
    if let Err(err) = provider::main() {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

#[cfg(not(windows))]
fn main() {
    eprintln!("lockbook-cloud-files runs on Windows only");
    std::process::exit(1);
}
