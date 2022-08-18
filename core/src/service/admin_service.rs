use crate::service::api_service;
use crate::service::api_service::ApiError;
use crate::{core_err_unexpected, CoreError, RequestContext};
use lockbook_shared::api::{AdminDeleteAccountError, AdminDeleteAccountRequest};

impl RequestContext<'_, '_> {
    pub fn delete_account(&self, username: &str) -> Result<(), CoreError> {
        let account = self.get_account()?;

        api_service::request(account, AdminDeleteAccountRequest { username: username.to_string() })
            .map_err(|err| match err {
                ApiError::Endpoint(AdminDeleteAccountError::UserNotFound) => {
                    CoreError::UsernameNotFound
                }
                ApiError::Endpoint(AdminDeleteAccountError::NotPermissioned) => {
                    CoreError::InsufficientPermission
                }
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })
    }
}
