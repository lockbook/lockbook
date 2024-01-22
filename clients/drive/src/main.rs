pub mod core;
pub mod fs_impl;
pub mod utils;

use core::AsyncCore;
use std::io::{self, IsTerminal};

use cli_rs::{
    cli_error::{CliError, CliResult},
    command::Command,
    parser::Cmd,
};
use nfsserve::tcp::{NFSTcp, NFSTcpListener};

pub static VERBOSE: bool = true;

// Test with
// mount -t nfs -o nolocks,vers=3,tcp,port=8000,mountport=8000,soft 127.0.0.1:/ mnt/
pub struct Drive {
    ac: AsyncCore,
}

fn main() {
    println!("test");
    Command::name("lbdrive")
        .subcommand(Command::name("import").handler(|| Drive::init().import()))
        .subcommand(Command::name("mount").handler(|| Drive::init().mount()))
        .parse();
}

impl Drive {
    /// executing this from within an async context will panic
    fn init() -> Self {
        let ac = AsyncCore::init();

        Self { ac }
    }

    #[tokio::main]
    async fn import(&self) -> CliResult<()> {
        if io::stdin().is_terminal() {
            return Err(CliError::from("to import an existing lockbook account, pipe your account string into this command, e.g.:\npbpaste | lockbook account import".to_string()));
        }

        let mut account_string = String::new();
        io::stdin()
            .read_line(&mut account_string)
            .expect("failed to read from stdin");
        account_string.retain(|c| !c.is_whitespace());

        println!("importing account...");
        self.ac.import_account(&account_string).await;

        self.ac.sync().await;

        Ok(())
    }

    #[tokio::main]
    async fn mount(self) -> CliResult<()> {
        // todo make this also handle the server mounting
        // todo pick a port that's unlikely to be used
        // todo have a better port selection strategy
        let listener = NFSTcpListener::bind(&format!("127.0.0.1:11111"), self)
            .await
            .unwrap();

        listener.handle_forever().await.unwrap();
        Ok(())
    }
}
