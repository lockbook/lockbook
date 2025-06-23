impl Lb {
    pub async fn share_file(&self, id: Uuid, username: &str, mode: ShareMode) -> LbResult<()>{
        match self {
            Lb::Direct(inner) => {
                inner.share_file(id,username,mode).await
            }
            Lb::Network(proxy) => {
                proxy.share_file(id,username,mode).await
            }
        }
    }
    pub async fn get_pending_shares(&self) -> LbResult<Vec<File>>{
        match self {
            Lb::Direct(inner) => {
                inner.get_pending_shares().await
            }
            Lb::Network(proxy) => {
                proxy.get_pending_shares().await
            }
        }
    }
    pub async fn reject_share(&self, id: &Uuid) -> Result<(), LbErr>{
        match self {
            Lb::Direct(inner) => {
                inner.reject_share(id).await
            }
            Lb::Network(proxy) => {
                proxy.reject_share(id).await
            }
        }
    }
}

use uuid::Uuid;
use crate::{model::{errors::LbErr, file::{File, ShareMode}}, Lb, LbResult};