use std::env;

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, Copy, PartialEq, Eq, Default)]
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

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
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
}

impl Config {
    /// Configures lockbook for CLI use with no stdout logs or background work. `writeable_path_subfolder` is generally
    /// a hardcoded client name like `"cli"`.
    pub fn cli_config(writeable_path_subfolder: &str) -> Config {
        Config {
            writeable_path: Self::writeable_path(writeable_path_subfolder),
            background_work: false,
            logs: true,
            stdout_logs: false,
            colored_logs: true,
            client_type: ClientType::Cli,
        }
    }

    /// Configures lockbook for UI use with stdout logs and background work. `writeable_path_subfolder` is generally
    /// a hardcoded client name like `"macos"`.
    pub fn ui_config(writeable_path_subfolder: &str) -> Config {
        Config {
            writeable_path: Self::writeable_path(writeable_path_subfolder),
            background_work: true,
            logs: true,
            stdout_logs: true,
            colored_logs: true,
            client_type: ClientType::Ui,
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
