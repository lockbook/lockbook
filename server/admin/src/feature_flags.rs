use crate::FeatureFlag;
use lockbook_server_lib::feature_flags::FEATURE_FLAG_NEW_ACCOUNTS_FIELD;
use lockbook_server_lib::ServerState;

pub async fn toggle_new_account_feature_flag(
    server_state: ServerState,
    feature_flag: Option<FeatureFlag>,
) -> bool {
    let mut con = server_state.index_db_pool.get().await.unwrap();

    match feature_flag {
        Some(FeatureFlag::NewAccount { enable }) => {
            lockbook_server_lib::feature_flags::set_new_account_status(&mut con, enable)
                .await
                .unwrap()
        }
        None => {
            println!(
                "{}: {}",
                FEATURE_FLAG_NEW_ACCOUNTS_FIELD,
                lockbook_server_lib::feature_flags::is_new_accounts_enabled(&mut con)
                    .await
                    .unwrap()
            )
        }
    }

    true
}
