use crate::{keys, ServerError, ServerState};
use redis::AsyncCommands;
use std::fmt::Debug;
use uuid::Uuid;

const YEAR: usize = 31536000;

pub async fn create<T: Debug>(
    state: &ServerState,
    id: Uuid,
    content_version: u64,
    content: &[u8],
) -> Result<(), ServerError<T>> {
    let key = &keys::doc(id, content_version);
    let mut con = state.index_db_pool.get().await?;
    con.set_ex(key, content, YEAR).await?;
    Ok(())
}

pub async fn delete<T: Debug>(
    state: &ServerState,
    id: Uuid,
    content_version: u64,
) -> Result<(), ServerError<T>> {
    let key = &keys::doc(id, content_version);
    let mut con = state.index_db_pool.get().await?;
    con.del(key).await?;
    Ok(())
}

pub async fn get<T: Debug>(
    state: &ServerState,
    id: Uuid,
    content_version: u64,
) -> Result<Option<Vec<u8>>, ServerError<T>> {
    let key = &keys::doc(id, content_version);
    let mut con = state.index_db_pool.get().await?;
    let data: Option<Vec<u8>> = con.get(&key).await?;
    Ok(data)
}
