use lb_rs::io::network::ApiError;
use lb_rs::io::server::Server;
use lb_rs::model::api::{
    FREE_TIER_USAGE_SIZE, GetSubscriptionInfoRequest, PREMIUM_TIER_USAGE_SIZE, PaymentMethod,
    PaymentPlatform, StripeAccountState, StripeAccountTier, UpgradeAccountStripeError,
    UpgradeAccountStripeRequest,
};
use lb_rs::model::errors::LbErrKind;
use lb_rs::model::file::ShareMode;
use lb_rs::model::file_metadata::{FileType, Owner};
use lb_rs::model::pubkey;
use test_utils::lockbook_server_lib::billing::billing_model::{BillingPlatform, StripeUserInfo};
use test_utils::lockbook_server_lib::guard::ServerTx;
use test_utils::*;
use uuid::Uuid;

async fn set_stripe_state(core: &Lb, account_state: StripeAccountState) {
    let owner = Owner(core.get_account().unwrap().public_key());
    let mut db = core.client.index_db.lock().await;
    let mut tx = ServerTx::begin(&mut db).unwrap();
    let mut account = tx.accounts.get(&owner).unwrap().clone();
    account.billing_info.billing_platform = Some(BillingPlatform::Stripe(StripeUserInfo {
        customer_id: String::new(),
        customer_name: Uuid::new_v4(),
        price_id: String::new(),
        payment_method_id: String::new(),
        last_4: "4242".to_string(),
        subscription_id: String::new(),
        expiration_time: 0,
        account_state,
    }));
    tx.accounts.insert(owner, account).unwrap();
    tx.end().unwrap();
}

async fn free_core() -> Lb {
    let core = test_core().await;
    core.create_account(&random_name()[..16], &url(), false)
        .await
        .unwrap();
    core
}

async fn premium_core() -> Lb {
    let core = free_core().await;
    set_stripe_state(&core, StripeAccountState::Ok).await;
    core
}

async fn sign_in(parent: &Lb, username: &str) -> Lb {
    let key = parent
        .export_sub_account_private_key(username)
        .await
        .unwrap();
    let core = test_core().await;
    core.import_account(&key, Some(&url())).await.unwrap();
    core.sync().await.unwrap();
    core
}

async fn write_random(core: &Lb, path: &str, bytes: u64) {
    let document = core.create_at_path(path).await.unwrap();
    let content: Vec<u8> = (0..bytes).map(|_| rand::random::<u8>()).collect();
    core.write_document(document.id, &content).await.unwrap();
}

#[tokio::test]
async fn free_accounts_cannot_create() {
    let core = free_core().await;

    let result = core.create_sub_account("bot").await;

    assert_eq!(result.unwrap_err().kind, LbErrKind::NotPremium);
}

#[tokio::test]
async fn create_then_sign_in() {
    let parent = premium_core().await;
    let parent_account = parent.get_account().unwrap();

    let username = parent.create_sub_account("Bot").await.unwrap();
    assert_eq!(username, format!("{}+bot", parent_account.username));

    let listed = parent.list_sub_accounts().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].username, username);
    assert_eq!(listed[0].counter, 0);

    let sub_account = sign_in(&parent, &username).await;
    assert_eq!(sub_account.get_account().unwrap().username, username);
    assert_eq!(sub_account.root().await.unwrap().name, username);
    assert_eq!(
        sub_account.get_account().unwrap().private_key,
        pubkey::derive_key(&parent_account.private_key, 0).unwrap()
    );

    let phrase = parent.export_sub_account_phrase(&username).await.unwrap();
    assert_eq!(phrase, sub_account.export_account_phrase().unwrap());
}

#[tokio::test]
async fn each_gets_its_own_key() {
    let parent = premium_core().await;

    let first = parent.create_sub_account("first").await.unwrap();
    let second = parent.create_sub_account("second").await.unwrap();

    let listed = parent.list_sub_accounts().await.unwrap();
    assert_eq!(listed.iter().map(|s| s.counter).collect::<Vec<_>>(), vec![0, 1]);
    assert_ne!(
        parent.export_sub_account_private_key(&first).await.unwrap(),
        parent
            .export_sub_account_private_key(&second)
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn names() {
    let parent = premium_core().await;
    parent.create_sub_account("bot").await.unwrap();

    for label in ["", "a+b", "a b", "a-label-too-long-to-fit-in-a-username"] {
        let result = parent.create_sub_account(label).await;
        assert_eq!(result.unwrap_err().kind, LbErrKind::UsernameInvalid, "{label}");
    }

    let result = parent.create_sub_account("bot").await;
    assert_eq!(result.unwrap_err().kind, LbErrKind::UsernameTaken);

    let squatter = test_core().await;
    let name = format!("{}+bot2", parent.get_account().unwrap().username);
    let result = squatter.create_account(&name, &url(), false).await;
    assert_eq!(result.unwrap_err().kind, LbErrKind::UsernameInvalid);
}

#[tokio::test]
async fn no_nesting() {
    let parent = premium_core().await;
    let username = parent.create_sub_account("bot").await.unwrap();
    let sub_account = sign_in(&parent, &username).await;

    let result = sub_account.create_sub_account("nested").await;

    assert_eq!(result.unwrap_err().kind, LbErrKind::InsufficientPermission);
    assert!(sub_account.list_sub_accounts().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_grant_is_a_share() {
    let parent = premium_core().await;
    let username = parent.create_sub_account("bot").await.unwrap();
    let sub_account = sign_in(&parent, &username).await;

    let folder = parent.create_at_path("granted/").await.unwrap();
    parent
        .share_file(folder.id, &username, ShareMode::Write)
        .await
        .unwrap();
    parent.sync().await.unwrap();
    sub_account.sync().await.unwrap();

    let pending = sub_account.get_pending_shares().await.unwrap();
    assert_eq!(pending.len(), 1);
    let document = sub_account
        .create_file("note.md", &pending[0].id, FileType::Document)
        .await
        .unwrap();
    sub_account
        .write_document(document.id, b"from the sub-account")
        .await
        .unwrap();
    sub_account.sync().await.unwrap();
    parent.sync().await.unwrap();

    assert_eq!(parent.read_document(document.id, false).await.unwrap(), b"from the sub-account");
    let own_usage = sub_account.get_usage().await.unwrap().server_usage.exact;
    assert_eq!(own_usage, lb_rs::model::api::METADATA_FEE);
}

#[tokio::test]
async fn usage_counts_against_the_parent() {
    let parent = premium_core().await;
    let username = parent.create_sub_account("bot").await.unwrap();
    let sub_account = sign_in(&parent, &username).await;

    write_random(&sub_account, "mine.md", 1000).await;
    sub_account.sync().await.unwrap();

    let parent_usage = parent.get_usage().await.unwrap();
    let sub_usage = sub_account.get_usage().await.unwrap();
    assert_eq!(parent_usage.data_cap.exact, PREMIUM_TIER_USAGE_SIZE - sub_usage.server_usage.exact);
    assert_eq!(sub_usage.data_cap.exact, PREMIUM_TIER_USAGE_SIZE - parent_usage.server_usage.exact);
    assert_eq!(parent.list_sub_accounts().await.unwrap()[0].usage, sub_usage.server_usage.exact);
}

#[tokio::test]
async fn the_family_shares_one_cap() {
    let parent = premium_core().await;
    let username = parent.create_sub_account("bot").await.unwrap();
    let sub_account = sign_in(&parent, &username).await;
    set_stripe_state(&parent, StripeAccountState::Canceled).await;

    let most_of_the_cap = (FREE_TIER_USAGE_SIZE as f64 * 0.6) as u64;
    write_random(&parent, "parent.md", most_of_the_cap).await;
    parent.sync().await.unwrap();

    write_random(&sub_account, "sub.md", most_of_the_cap).await;
    let result = sub_account.sync().await;
    assert_eq!(result.unwrap_err().kind, LbErrKind::UsageIsOverDataCap);

    let document = parent.get_by_path("parent.md").await.unwrap();
    parent.delete(&document.id).await.unwrap();
    parent.sync().await.unwrap();

    sub_account.sync().await.unwrap();
}

#[tokio::test]
async fn billing_follows_the_parent() {
    let parent = premium_core().await;
    let username = parent.create_sub_account("bot").await.unwrap();
    let sub_account = sign_in(&parent, &username).await;
    let account = sub_account.get_account().unwrap();

    let info =
        Server::get_subscription_info(&sub_account.client, account, GetSubscriptionInfoRequest {})
            .await
            .unwrap()
            .subscription_info
            .unwrap();
    assert_eq!(
        info.payment_platform,
        PaymentPlatform::Stripe { card_last_4_digits: String::new() }
    );

    let result = Server::upgrade_account_stripe(
        &sub_account.client,
        account,
        UpgradeAccountStripeRequest {
            account_tier: StripeAccountTier::Premium(PaymentMethod::OldCard),
        },
    )
    .await;
    assert!(matches!(result, Err(ApiError::Endpoint(UpgradeAccountStripeError::AlreadyPremium))));

    let result = sub_account.cancel_subscription().await;
    assert_eq!(result.unwrap_err().kind, LbErrKind::NotPremium);
}

#[tokio::test]
async fn parent_deletes() {
    let parent = premium_core().await;
    let username = parent.create_sub_account("bot").await.unwrap();
    let sub_account = sign_in(&parent, &username).await;
    write_random(&sub_account, "mine.md", 1000).await;
    sub_account.sync().await.unwrap();

    parent.delete_sub_account(&username).await.unwrap();

    assert!(parent.list_sub_accounts().await.unwrap().is_empty());
    assert_eq!(parent.get_usage().await.unwrap().data_cap.exact, PREMIUM_TIER_USAGE_SIZE);
    let result = parent.delete_sub_account(&username).await;
    assert_eq!(result.unwrap_err().kind, LbErrKind::UsernameNotFound);

    let result = parent.create_sub_account("bot").await;
    assert_eq!(result.unwrap_err().kind, LbErrKind::UsernameTaken);

    let replacement = parent.create_sub_account("bot2").await.unwrap();
    assert_eq!(parent.list_sub_accounts().await.unwrap()[0].counter, 1);
    assert_ne!(
        parent
            .export_sub_account_private_key(&replacement)
            .await
            .unwrap(),
        sub_account.export_account_private_key().unwrap()
    );
}

#[tokio::test]
async fn sub_account_deletes_itself() {
    let parent = premium_core().await;
    let username = parent.create_sub_account("bot").await.unwrap();
    let sub_account = sign_in(&parent, &username).await;

    sub_account.delete_account().await.unwrap();

    assert!(parent.list_sub_accounts().await.unwrap().is_empty());
}

#[tokio::test]
async fn deleting_the_parent_deletes_its_sub_accounts() {
    let parent = premium_core().await;
    let username = parent.create_sub_account("bot").await.unwrap();
    let sub_account = sign_in(&parent, &username).await;
    let owner = Owner(sub_account.get_account().unwrap().public_key());
    let server = sub_account.client.clone();

    parent.delete_account().await.unwrap();

    let db = server.index_db.lock().await;
    assert!(
        db.schema
            .owned_files
            .get(&owner)
            .is_none_or(|ids| ids.is_empty())
    );
    assert!(
        db.schema
            .sub_accounts_by_parent
            .get(&Owner(parent.get_account().unwrap().public_key()))
            .is_none_or(|s| s.is_empty())
    );
}
