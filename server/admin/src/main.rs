mod delete_account;

use crate::delete_account::delete_account;
use crate::Subcommands::DeleteAccount;
use deadpool_redis::Pool;

use deadpool_redis::Runtime;
use lockbook_server_lib::config::Config;
use lockbook_server_lib::{ ServerState};

use s3::bucket::Bucket;
use structopt::StructOpt;
use lockbook_server_lib::content::file_content_client;

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(about = "A utility for a lockbook server administrator.")]
enum Subcommands {
    /// Purge a user, and all their files from postgres & s3
    ///  *Note: This is intentionally left unexposed to give the user experience of deleting a user more
    /// thought. This includes thinking about being able to mark themselves as compromised and indicate to
    /// collaborators that certain files are potentially compromised. This could also involve us reaching out
    /// to services like Stripe / Apple / Google and terminating open subscriptions.
    /// Additionally deleted usernames should not be "freed". Usernames are a form of identity that's
    /// immutable, if a username is compromised or deleted, it is consumed forever, someone else cannot
    /// assume that identity.
    DeleteAccount { username: String },
}

#[tokio::main]
async fn main() {
    let config = Config::from_env_vars();
    let (index_db_pool, files_db_client) = connect_to_state(&config).await;
    let server_state = ServerState {
        config,
        index_db_pool,
        files_db_client,
    };

    let ok = match Subcommands::from_args() {
        DeleteAccount { username: user } => delete_account(server_state, &user).await,
    };

    if ok {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}

async fn connect_to_state(config: &Config) -> (Pool, Bucket) {
    let index_db_pool = deadpool_redis::Config::from_url(&config.index_db.redis_url)
        .create_pool(Some(Runtime::Tokio1));
    let files_db = file_content_client::create_client(&config.files_db);
    (index_db_pool.unwrap(), files_db.unwrap())
}
