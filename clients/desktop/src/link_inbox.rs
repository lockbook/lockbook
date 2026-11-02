//! Hands a link from a second launch to the app already open on the same
//! data directory: the first app listens, a launch with a link sends it and
//! exits.

use std::io::Write as _;

/// Sends `url` to the app open on this data directory, if one is.
pub fn send(url: &str) -> bool {
    match connect() {
        Ok(mut peer) => peer.write_all(url.as_bytes()).is_ok(),
        Err(_) => false,
    }
}

/// Calls `on_link` with each link a later launch sends, from a thread of
/// its own.
pub fn listen(on_link: impl Fn(String) + Send + 'static) {
    std::thread::spawn(move || {
        // a second app on the same data directory leaves the links to the first
        if connect().is_ok() {
            return;
        }
        if let Err(err) = serve(&on_link) {
            log::warn!("links from other launches won't arrive: {err}");
        }
    });
}

fn received(mut peer: impl std::io::Read, on_link: &impl Fn(String)) {
    let mut url = String::new();
    if peer.read_to_string(&mut url).is_ok() && !url.is_empty() {
        on_link(url);
    }
}

#[cfg(unix)]
fn address() -> std::path::PathBuf {
    let dir = lb::model::core_config::Config::writeable_path("egui");
    std::path::Path::new(&dir).join("desktop.sock")
}

#[cfg(unix)]
fn connect() -> std::io::Result<std::os::unix::net::UnixStream> {
    std::os::unix::net::UnixStream::connect(address())
}

#[cfg(unix)]
fn serve(on_link: &impl Fn(String)) -> std::io::Result<()> {
    let address = address();
    // nobody answered, so a socket left here is from an app that quit
    let _ = std::fs::remove_file(&address);
    let listener = std::os::unix::net::UnixListener::bind(&address)?;
    for peer in listener.incoming().flatten() {
        received(peer, on_link);
    }
    Ok(())
}

#[cfg(windows)]
fn address() -> Vec<u16> {
    let dir = lb::model::core_config::Config::writeable_path("egui");
    // a pipe name takes any character but a backslash
    let name = format!(r"\\.\pipe\lockbook-desktop-{}", dir.replace('\\', "/"));
    name.encode_utf16().chain([0]).collect()
}

#[cfg(windows)]
fn connect() -> std::io::Result<std::fs::File> {
    let address = address();
    let address = String::from_utf16_lossy(&address[..address.len() - 1]);
    let pipe = std::fs::OpenOptions::new().write(true).open(address)?;
    // the open app may come forward, as this launch is the one the user started
    unsafe { windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(u32::MAX) };
    Ok(pipe)
}

#[cfg(windows)]
fn serve(on_link: &impl Fn(String)) -> std::io::Result<()> {
    use std::os::windows::io::FromRawHandle as _;
    use windows_sys::Win32::Foundation::{
        ERROR_PIPE_CONNECTED, GetLastError, HANDLE, INVALID_HANDLE_VALUE,
    };
    use windows_sys::Win32::Storage::FileSystem::{
        FILE_FLAG_FIRST_PIPE_INSTANCE, PIPE_ACCESS_INBOUND,
    };
    use windows_sys::Win32::System::Pipes::{
        ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
        PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
    };

    let address = address();
    let create = |access: u32| -> std::io::Result<HANDLE> {
        let mode = PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS;
        let pipe = unsafe {
            CreateNamedPipeW(
                address.as_ptr(),
                access,
                mode,
                PIPE_UNLIMITED_INSTANCES,
                0,
                4096,
                0,
                std::ptr::null(),
            )
        };
        match pipe == INVALID_HANDLE_VALUE {
            true => Err(std::io::Error::last_os_error()),
            false => Ok(pipe),
        }
    };
    // the first instance fails if another process already owns the name
    let mut pipe = create(PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE)?;
    loop {
        // a peer that connects before this call shows as ERROR_PIPE_CONNECTED
        let connected = unsafe { ConnectNamedPipe(pipe, std::ptr::null_mut()) } != 0
            || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
        // the next sender finds an instance waiting while this one is read
        let next = create(PIPE_ACCESS_INBOUND)?;
        let peer = unsafe { std::fs::File::from_raw_handle(pipe as _) };
        if connected {
            received(peer, on_link);
        }
        pipe = next;
    }
}
