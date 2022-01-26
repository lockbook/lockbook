use crate::FeatureFlag;
use lockbook_server_lib::keys::{FEATURE_FLAGS_KEY, FEATURE_FLAG_NEW_ACCOUNTS_FIELD};
use lockbook_server_lib::ServerState;
use redis::AsyncCommands;

pub async fn toggle_new_account_feature_flag(
    server_state: ServerState,
    feature_flag: FeatureFlag,
) -> bool {
    let mut con = server_state.index_db_pool.get().await.unwrap();

    match feature_flag {
        FeatureFlag::NewAccount { enable } => {
            con.hset::<_, _, bool, bool>(
                FEATURE_FLAGS_KEY,
                FEATURE_FLAG_NEW_ACCOUNTS_FIELD,
                enable,
            )
            .await
            .expect("Could not enable/disable new account creation.");
        }
    }

    true
}
