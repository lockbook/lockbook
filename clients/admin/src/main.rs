mod account;
mod disappear;
mod info;
mod validate;

use cli_rs::arg::Arg;
use cli_rs::cli_error::{CliResult, Exit};
use cli_rs::command::Command;
use cli_rs::flag::Flag;
use cli_rs::parser::Cmd;
use lb::Uuid;
use lb::blocking::Lb;
use lb::model::api::{
    AdminSetUserTierInfo, AppStoreAccountState, GooglePlayAccountState, ServerIndex,
};
use lb::model::core_config::Config;

fn main() {
    Command::name("admin")
        .description("Lockbook server administration, as the admin account the CLI is signed in to.")
        .with_completions()
        .subcommand(
            Command::name("disappear-account")
                .description("Disappear a user, freeing up their username")
                .input(Arg::str("username"))
                .handler(|username| disappear::account(&core()?, username.get())),
        )
        .subcommand(
            Command::name("disappear-file")
                .description("Disappear a file so that it never existed, for when the server let in an invalid file")
                .input(Arg::<Uuid>::name("id"))
                .handler(|id| disappear::file(&core()?, id.get())),
        )
        .subcommand(
            Command::name("validate-account")
                .description("Validate a user's file tree and print any failures")
                .input(Arg::str("username"))
                .handler(|username| validate::account(&core()?, username.get())),
        )
        .subcommand(
            Command::name("validate-server")
                .description("Perform server-wide integrity checks")
                .handler(|| validate::server(&core()?)),
        )
        .subcommand(
            Command::name("list-users")
                .description("List all users, or only premium ones")
                .input(Flag::bool("premium"))
                .input(Flag::bool("app-store-premium"))
                .input(Flag::bool("google-play-premium"))
                .input(Flag::bool("stripe-premium"))
                .handler(|premium, app_store, google_play, stripe| {
                    account::list(
                        &core()?,
                        premium.get(),
                        app_store.get(),
                        google_play.get(),
                        stripe.get(),
                    )
                }),
        )
        .subcommand(
            Command::name("account-info")
                .description("Print a user's username, root, payment platform, usage, creation and last-seen dates, and whether they deleted their account")
                .input(Flag::<String>::new("username"))
                .input(
                    Flag::<String>::new("public-key")
                        .description("base64 encoded and compressed"),
                )
                .handler(|username, public_key| {
                    account::info(&core()?, username.get(), public_key.get())
                }),
        )
        .subcommand(
            Command::name("rebuild-index")
                .description("Rebuild one of the server's indexes")
                .subcommand(
                    Command::name("owned-files")
                        .handler(|| rebuild_index(ServerIndex::OwnedFiles)),
                )
                .subcommand(
                    Command::name("shared-files")
                        .handler(|| rebuild_index(ServerIndex::SharedFiles)),
                )
                .subcommand(
                    Command::name("file-children")
                        .handler(|| rebuild_index(ServerIndex::FileChildren)),
                ),
        )
        .subcommand(
            Command::name("file-info")
                .description("Print a file as it appears on the server")
                .input(Arg::<Uuid>::name("id"))
                .handler(|id| info::file(&core()?, id.get())),
        )
        .subcommand(
            Command::name("set-user-tier")
                .description("Manually set a user's tier and their subscription information")
                .subcommand(
                    Command::name("stripe")
                        .description("asks for the Stripe details one at a time")
                        .input(Arg::str("username"))
                        .handler(|username| account::set_stripe_tier(&core()?, username.get())),
                )
                .subcommand(
                    Command::name("google-play")
                        .input(Arg::str("username"))
                        .input(Arg::str("purchase-token"))
                        .input(Arg::<u64>::name("expiration-time"))
                        .input(Arg::<GooglePlayAccountState>::name("account-state"))
                        .handler(|username, purchase_token, expiration_time, account_state| {
                            account::set_user_tier(
                                &core()?,
                                username.get(),
                                AdminSetUserTierInfo::GooglePlay {
                                    purchase_token: purchase_token.get(),
                                    expiration_time: expiration_time.get(),
                                    account_state: account_state.get(),
                                },
                            )
                        }),
                )
                .subcommand(
                    Command::name("app-store")
                        .input(Arg::str("username"))
                        .input(Arg::str("account-token"))
                        .input(Arg::str("original-transaction-id"))
                        .input(Arg::<u64>::name("expiration-time"))
                        .input(Arg::<AppStoreAccountState>::name("account-state"))
                        .handler(
                            |username,
                             account_token,
                             original_transaction_id,
                             expiration_time,
                             account_state| {
                                account::set_user_tier(
                                    &core()?,
                                    username.get(),
                                    AdminSetUserTierInfo::AppStore {
                                        account_token: account_token.get(),
                                        original_transaction_id: original_transaction_id.get(),
                                        expiration_time: expiration_time.get(),
                                        account_state: account_state.get(),
                                    },
                                )
                            },
                        ),
                )
                .subcommand(
                    Command::name("free")
                        .input(Arg::str("username"))
                        .handler(|username| {
                            account::set_user_tier(
                                &core()?,
                                username.get(),
                                AdminSetUserTierInfo::Free,
                            )
                        }),
                ),
        )
        .parse()
        .exit();
}

fn core() -> CliResult<Lb> {
    Ok(Lb::init(Config { client_name: "admin".into(), ..Config::cli_config("cli") })?)
}

fn rebuild_index(index: ServerIndex) -> CliResult<()> {
    core()?.admin_rebuild_index(index)?;
    Ok(())
}
