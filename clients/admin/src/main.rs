mod account;
mod disappear;
mod error;
mod indexes;
mod info;
mod validate;

use clap::{Parser, Subcommand};
use lb::Uuid;
use lb::blocking::Lb;
use lb::model::api::UnixTimeMillis;
use lb::model::core_config::Config;

use crate::error::Error;
use crate::indexes::CliIndex;

#[derive(Debug, PartialEq, Eq, Parser)]
pub enum Admin {
    /// Disappear a user
    ///
    /// Frees up their username
    DisappearAccount { username: String },

    /// Disappear a file
    ///
    /// When you delete a file you flip that file's is_deleted flag to false. In a disaster recovery
    /// scenario, you may want to *disappear* a file so that it never existed. This is useful in a
    /// scenario where your server let in an invalid file.
    DisappearFile { id: Uuid },

    /// Validates file trees of all users on the server and prints any failures
    ValidateAccount { username: String },

    /// Performs server-wide integrity checks
    ValidateServer,

    /// List all users
    ListUsers {
        #[structopt(short, long)]
        premium: bool,

        #[structopt(short, long)]
        app_store_premium: bool,

        #[structopt(short, long)]
        google_play_premium: bool,

        #[structopt(short, long)]
        stripe_premium: bool,
    },

    /// Get a user's info. This includes their username, public key, and payment platform.
    AccountInfo {
        #[structopt(short, long)]
        username: Option<String>,

        // A base 64 encoded and compressed public key
        #[structopt(short, long)]
        public_key: Option<String>,
    },

    #[command(subcommand)]
    RebuildIndex(CliIndex),

    /// Prints information about a file as it appears on the server
    FileInfo { id: Uuid },

    /// Manually set a user's tier and their subscription information
    #[command(subcommand)]
    SetUserTier(SetUserTier),
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub enum SetUserTier {
    Stripe {
        username: String,
        customer_id: String,
        customer_name: Uuid,
        payment_method_id: String,
        last_4: String,
        subscription_id: String,
        expiration_time: UnixTimeMillis,
        account_state: String,
    },

    GooglePlay {
        username: String,
        purchase_token: String,
        expiration_time: UnixTimeMillis,
        account_state: String,
    },

    AppStore {
        username: String,
        account_token: String,
        original_transaction_id: String,
        expiration_time: UnixTimeMillis,
        account_state: String,
    },

    Free {
        username: String,
    },
}

type Res<T> = Result<T, Error>;

pub fn main() {
    let core = Lb::init(Config::cli_config("cli")).unwrap(); // use the cli account

    let result = match Admin::parse() {
        Admin::DisappearAccount { username } => disappear::account(&core, username),
        Admin::ListUsers { premium, app_store_premium, google_play_premium, stripe_premium } => {
            account::list(&core, premium, app_store_premium, google_play_premium, stripe_premium)
        }
        Admin::AccountInfo { username, public_key } => account::info(&core, username, public_key),
        Admin::DisappearFile { id } => disappear::file(&core, id),
        Admin::ValidateAccount { username } => validate::account(&core, username),
        Admin::ValidateServer => validate::server(&core),
        Admin::FileInfo { id } => info::file(&core, id),
        Admin::RebuildIndex(index) => indexes::rebuild(&core, index),
        Admin::SetUserTier(info) => account::set_user_tier(&core, info),
    };

    if result.is_err() {
        panic!("unsuccessful completion")
    }
}
