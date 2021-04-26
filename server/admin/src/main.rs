mod delete_account;
mod loggers;

#[macro_use]
extern crate log;

use crate::delete_account::delete_account;
use crate::Subcommands::DeleteAccount;

use lockbook_server_lib::config::Config;
use lockbook_server_lib::{file_content_client, file_index_repo, ServerState};

use rsa::{BigUint, RSAPublicKey};
use s3::bucket::Bucket;
use structopt::StructOpt;
use tokio::join;
use tokio_postgres::Client as PostgresClient;

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(about = "A utility for a lockbook server administrator.")]
enum Subcommands {
    /// Purge a user, and all their files from postgres & s3
    ///  *Note: This is intentionally left unexposed to give the user experience of deleting a user more
    /// thought. This includes thinking about being able to mark themselves as compromised and indicate to
    /// collaborators that certain files are potentially compromised. This could also involve us reaching out
    /// to services like Strip / Apple / Google and terminating open subscriptions.
    /// Additionally deleted usernames should not be "freed". Usernames are a form of identity that's
    /// immutable, if a username is compromised or deleted, it is consumed forever, someone else cannot
    /// assume that identity.
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
