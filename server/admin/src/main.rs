mod delete_account;

use crate::delete_account::delete_account;
use crate::Subcommands::DeleteAccount;


use hmdb::log::Reader;
use lockbook_server_lib::billing::google_play_client::get_google_play_client;
use lockbook_server_lib::config::Config;
use lockbook_server_lib::schema::ServerV1;
use lockbook_server_lib::ServerState;

use structopt::StructOpt;

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

#[derive(Debug, PartialEq, StructOpt)]
#[structopt(about = "Toggleable features for lockbook server.")]
pub enum FeatureFlag {
    NewAccount {
        /// Enable or disable new accounts from being created.
        #[structopt(parse(try_from_str))]
        enable: bool,
    },
}

#[tokio::main]
async fn main() {
    let config = Config::from_env_vars();
    let stripe_client = stripe::Client::new(&config.billing.stripe.stripe_secret);
    let google_play_client =
        get_google_play_client(&config.billing.google.service_account_key).await;
    let index_db = ServerV1::init(&config.index_db.db_location).expect("Failed to load index_db");

    let server_state = ServerState { config, index_db, stripe_client, google_play_client };

    let ok = match Subcommands::from_args() {
        DeleteAccount { username: user } => delete_account(server_state, &user).await,
    };

    if ok {
        std::process::exit(0);
    } else {
        std::process::exit(1);
    }
}
