mod delete_account;
mod loggers;

#[macro_use]
extern crate log;

use crate::delete_account::delete_account;
use crate::Subcommands::DeleteAccount;

use lockbook_server_lib::config::Config;
use lockbook_server_lib::{file_content_client, file_index_repo, ServerState};

use s3::bucket::Bucket;
use structopt::StructOpt;
use tokio::join;
use tokio_postgres::Client as PostgresClient;
use rsa::{BigUint, RSAPublicKey};

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(about = "A utility for a lockbook server administrator.")]
enum Subcommands {
    /// Purge a user, and all their files from postgres & s3
    DeleteAccount { username: String },
}

#[tokio::main]
async fn main() {
    loggers::init();
    let config = Config::from_env_vars();
    let (index_db_client, files_db_client) = connect_to_state(&config).await;
    let server_state = ServerState {
        config,
        index_db_client,
        files_db_client,
    };

    match Subcommands::from_args() {
        DeleteAccount { username: user } => delete_account(server_state, &user).await,
    }
}

async fn connect_to_state(config: &Config) -> (PostgresClient, Bucket) {
    let index_db = file_index_repo::connect(&config.index_db);
    let files_db = file_content_client::connect(&config.files_db);
    let (index_db, files_db) = join!(index_db, files_db);
    (index_db.unwrap(), files_db.unwrap())
}
