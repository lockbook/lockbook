use crate::ServerState;
use redis::{AsyncCommands, RedisResult};

pub const FEATURE_FLAGS_KEY: &str = "feature_flags";

pub const FEATURE_FLAG_NEW_ACCOUNTS_FIELD: &str = "new_account";

pub async fn initialize_flags(state: &ServerState) {
    let mut con = state.index_db_pool.get().await.unwrap();

    if !con
        .hexists::<_, _, bool>(FEATURE_FLAGS_KEY, FEATURE_FLAG_NEW_ACCOUNTS_FIELD)
        .await
        .unwrap()
    {
        set_new_account_status(&mut con, true).await.unwrap();
    }
}

pub async fn is_new_accounts_enabled(con: &mut deadpool_redis::Connection) -> RedisResult<bool> {
    con.hget(FEATURE_FLAGS_KEY, FEATURE_FLAG_NEW_ACCOUNTS_FIELD)
        .await
}

pub async fn set_new_account_status(
    con: &mut deadpool_redis::Connection, enable: bool,
) -> RedisResult<()> {
    con.hset(FEATURE_FLAGS_KEY, FEATURE_FLAG_NEW_ACCOUNTS_FIELD, enable)
        .await
}
