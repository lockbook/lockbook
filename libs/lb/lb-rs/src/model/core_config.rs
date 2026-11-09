use std::env;

use serde::{Deserialize, Serialize};

use crate::{get_code_commit, get_code_version};

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
pub enum ClientType {
    Cli,
    Ui,
    #[default]
    Unknown,
}

impl ClientType {
    pub fn as_str(self) -> &'static str {
        match self {
            ClientType::Cli => "cli",
            ClientType::Ui => "ui",
            ClientType::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct Config {
    /// Identifies the client in startup logs and `lockbook-<client_name>.log`.
    #[serde(default)]
    pub client_name: String,

    /// Optional build identity of a client compiled separately from Rust.
    #[serde(default)]
    pub build_info: Option<String>,

    #[serde(default)]
    pub os_version: Option<String>,

    #[serde(default = "default_lb_version")]
    pub lb_version: String,

    #[serde(default = "default_lb_commit")]
    pub lb_commit: Option<String>,

    /// Where should lockbook store data, including logs?
    pub writeable_path: String,

    /// Should lb do background work like keep search indexes up to date?
    pub background_work: bool,

    /// Should we log at all?
    pub logs: bool,

    /// Should logs be printed to stdout?
    pub stdout_logs: bool,

    /// Should logs be colored?
    pub colored_logs: bool,

    #[serde(default)]
    pub client_type: ClientType,

    /// Should dev accounts exit when a db lock is held past its budget? Set per platform: a host
    /// turns it on where it enforces the frame budget. lb-rs can't derive it, since
    /// [ClientType::Ui] doesn't mean the host has a frame budget.
    #[serde(default)]
    pub lock_budget: bool,

    #[serde(default)]
    pub read_only: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            client_name: String::new(),
            build_info: None,
            os_version: None,
            lb_version: default_lb_version(),
            lb_commit: default_lb_commit(),
            writeable_path: String::new(),
            background_work: false,
            logs: false,
            stdout_logs: false,
            colored_logs: false,
            client_type: ClientType::Unknown,
            lock_budget: false,
            read_only: false,
        }
    }
}

fn default_lb_version() -> String {
    get_code_version().to_owned()
}

fn default_lb_commit() -> Option<String> {
    get_code_commit().map(str::to_owned)
}

impl Config {
    /// Configures lockbook for CLI use with no stdout logs or background work. `writeable_path_subfolder` is generally
    /// a hardcoded client name like `"cli"`.
    pub fn cli_config(writeable_path_subfolder: &str) -> Config {
        Config {
            client_name: writeable_path_subfolder.to_owned(),
            os_version: host_os_version(),
            writeable_path: Self::writeable_path(writeable_path_subfolder),
            logs: true,
            colored_logs: true,
            client_type: ClientType::Cli,
            ..Default::default()
        }
    }

    /// Configures lockbook for UI use with stdout logs and background work. `writeable_path_subfolder` is generally
    /// a hardcoded client name like `"macos"`.
    pub fn ui_config(writeable_path_subfolder: &str) -> Config {
        Config {
            client_name: writeable_path_subfolder.to_owned(),
            os_version: host_os_version(),
            writeable_path: Self::writeable_path(writeable_path_subfolder),
            background_work: true,
            logs: true,
            stdout_logs: true,
            colored_logs: true,
            client_type: ClientType::Ui,
            ..Default::default()
        }
    }

    /// Produces a full writable path for lockbook to use based on environment variables and platform. Useful for
    /// initializing the Config struct.
    pub fn writeable_path(writeable_path_subfolder: &str) -> String {
        if let Ok(specified_path) = env::var("LOCKBOOK_PATH") {
            return specified_path;
        }

        // `env::home_dir` is `HOME` then `getpwuid_r` on unix, and `USERPROFILE` then `GetUserProfileDirectory` on
        // windows. It notably never consults `HOME` on windows: we used to fall back to `HOMEPATH`, which omits the
        // drive letter (`\Users\parth`), so the data directory silently followed whichever drive the process happened
        // to be running from.
        let Some(home) = env::home_dir() else {
            panic!("no location for lockbook to initialize");
        };

        home.join(".lockbook")
            .join(writeable_path_subfolder)
            .to_string_lossy()
            .into_owned()
    }
}

// todo: we added background work as a flag to speed up test execution in debug mode
// turn background work back to true in test_utils to see the slow test
// the slow test primarily does a large amount of allocations due to ownership model
// of treelike. In a universe where these operations could be expressed as iterators
// we would be able to vastly cut down on allocations and eliminate this complexity
//
// another nice aspect of background work is that it is a workaround for CLI's lack
// of graceful shutdown. Ideally, both of these situations will be handled differently.

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub fn host_os_version() -> Option<String> {
    let mut buf = [0u8; 64];
    let mut len = buf.len();
    let res = unsafe {
        libc::sysctlbyname(
            c"kern.osproductversion".as_ptr(),
            buf.as_mut_ptr().cast(),
            &mut len,
            std::ptr::null_mut(),
            0,
        )
    };
    if res != 0 {
        return None;
    }
    let version = std::ffi::CStr::from_bytes_until_nul(&buf[..len])
        .ok()?
        .to_str()
        .ok()?;
    let os = if cfg!(target_os = "ios") { "iOS" } else { "macOS" };
    Some(format!("{os} {version}"))
}

#[cfg(target_os = "linux")]
pub fn host_os_version() -> Option<String> {
    let release = std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
        .ok()?;
    release
        .lines()
        .find_map(|line| line.strip_prefix("PRETTY_NAME="))
        .map(|name| name.trim_matches('"').to_owned())
}

#[cfg(windows)]
pub fn host_os_version() -> Option<String> {
    use windows_sys::Wdk::System::SystemServices::RtlGetVersion;
    use windows_sys::Win32::System::SystemInformation::OSVERSIONINFOW;

    let mut info: OSVERSIONINFOW = unsafe { std::mem::zeroed() };
    info.dwOSVersionInfoSize = std::mem::size_of::<OSVERSIONINFOW>() as u32;
    if unsafe { RtlGetVersion(&mut info) } != 0 {
        return None;
    }
    Some(format!("Windows {}.{}.{}", info.dwMajorVersion, info.dwMinorVersion, info.dwBuildNumber))
}

#[cfg(target_os = "android")]
pub fn host_os_version() -> Option<String> {
    fn property(name: &std::ffi::CStr) -> Option<String> {
        let mut buf = [0u8; libc::PROP_VALUE_MAX as usize];
        let len = unsafe { libc::__system_property_get(name.as_ptr(), buf.as_mut_ptr().cast()) };
        if len <= 0 {
            return None;
        }
        String::from_utf8(buf[..len as usize].to_vec()).ok()
    }
    let release = property(c"ro.build.version.release")?;
    match property(c"ro.build.version.sdk") {
        Some(sdk) => Some(format!("Android {release} (SDK {sdk})")),
        None => Some(format!("Android {release}")),
    }
}

#[cfg(not(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "linux",
    target_os = "android",
    windows
)))]
pub fn host_os_version() -> Option<String> {
    None
}
