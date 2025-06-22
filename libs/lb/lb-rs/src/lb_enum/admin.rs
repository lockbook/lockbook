impl Lb {
    pub async fn disappear_account(&self, username: &str) -> LbResult<()>{
        match self {
            Lb::Direct(inner) => {
                inner.disappear_account(username).await
            }
            Lb::Network(proxy) => {
                proxy.disappear_account(username).await
            }
        }
    }
    pub async fn disappear_file(&self, id: Uuid) -> LbResult<()>{
        match self {
            Lb::Direct(inner) => {
                inner.disappear_file(id).await
            }
            Lb::Network(proxy) => {
                proxy.disappear_file(id).await
            }
        }
    }
    pub async fn list_users(&self, filter: Option<AccountFilter>) -> LbResult<Vec<Username>>{
        match self {
            Lb::Direct(inner) => {
                inner.list_users(filter).await
            }
            Lb::Network(proxy) => {
                proxy.list_users(filter).await
            }
        }
    }
    pub async fn get_account_info(&self, identifier: AccountIdentifier) -> LbResult<AccountInfo>{
        match self {
            Lb::Direct(inner) => {
                inner.get_account_info(identifier).await
            }
            Lb::Network(proxy) => {
                proxy.get_account_info(identifier).await
            }
        }
    }
    pub async fn validate_account(&self, username: &str) -> LbResult<AdminValidateAccount> {
        match self {
            Lb::Direct(inner) => {
                inner.validate_account(username).await
            }
            Lb::Network(proxy) => {
                proxy.validate_account(username).await
            }
        }
    }
    pub async fn validate_server(&self) -> LbResult<AdminValidateServer>{
        match self {
            Lb::Direct(inner) => {
                inner.validate_server().await
            }
            Lb::Network(proxy) => {
                proxy.validate_server().await
            }
        }
    }
    pub async fn file_info(&self, id: Uuid) -> LbResult<AdminFileInfoResponse>{
        match self {
            Lb::Direct(inner) => {
                inner.file_info(id).await
            }
            Lb::Network(proxy) => {
                proxy.file_info(id).await
            }
        }
    }
    pub async fn rebuild_index(&self, index: ServerIndex) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => {
                inner.rebuild_index(index).await
            }
            Lb::Network(proxy) => {
                proxy.rebuild_index(index).await
            }
        }
    }
    pub async fn set_user_tier(&self, username: &str, info: AdminSetUserTierInfo) -> LbResult<()> {
        match self {
            Lb::Direct(inner) => {
                inner.set_user_tier(username,info).await
            }
            Lb::Network(proxy) => {
                proxy.set_user_tier(username,info).await
            }
        }
    }
}

use uuid::Uuid;

use crate::{model::{account::Username, api::{AccountFilter, AccountIdentifier, AccountInfo, AdminFileInfoResponse, AdminSetUserTierInfo, AdminValidateAccount, AdminValidateServer, ServerIndex}}, Lb, LbResult};