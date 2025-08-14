use std::env;

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
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

    /// Should lb listen and serve connections from client lbs?
    pub rpc_port: Option<u16>,
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
            rpc_port: Self::rpc_port(),
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
            rpc_port: Self::rpc_port(),
        }
    }

    /// Produces a full writable path for lockbook to use based on environment variables and platform. Useful for
    /// initializing the Config struct.
    pub fn writeable_path(writeable_path_subfolder: &str) -> String {
        let specified_path = env::var("LOCKBOOK_PATH");

        let default_path =
            env::var("HOME") // unix
                .or(env::var("HOMEPATH")) // windows
                .map(|home| format!("{home}/.lockbook/{writeable_path_subfolder}"));

        let Ok(writeable_path) = specified_path.or(default_path) else {
            panic!("no location for lockbook to initialize");
        };

        writeable_path
    }

    pub fn rpc_port() -> Option<u16> {
        match env::var("LOCKBOOK_RPC_PORT") {
            Ok(val) => val.parse::<u16>().ok(),
            Err(_) => None,
        }
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
