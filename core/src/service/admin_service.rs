use crate::service::api_service;
use crate::service::api_service::ApiError;
use crate::{core_err_unexpected, CoreError, CoreResult, RequestContext};
use lockbook_shared::account::Username;
use lockbook_shared::api::{
    AdminDeleteAccountError, AdminDeleteAccountRequest, AdminDisappearFileError,
    AdminDisappearFileRequest, AdminListPremiumUsersError, AdminListPremiumUsersRequest,
    ShallowPaymentPlatform,
};
use uuid::Uuid;

impl RequestContext<'_, '_> {
    pub fn delete_account(&self, username: &str) -> CoreResult<()> {
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

    pub fn disappear_file(&self, id: Uuid) -> CoreResult<()> {
        let account = self.get_account()?;
        api_service::request(account, AdminDisappearFileRequest { id }).map_err(|err| match err {
            ApiError::Endpoint(AdminDisappearFileError::FileNonexistent) => {
                CoreError::FileNonexistent
            }
            ApiError::Endpoint(AdminDisappearFileError::NotPermissioned) => {
                CoreError::InsufficientPermission
            }
            ApiError::SendFailed(_) => CoreError::ServerUnreachable,
            ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
            _ => core_err_unexpected(err),
        })
    }

    pub fn list_premium_users(&self) -> CoreResult<Vec<(Username, ShallowPaymentPlatform)>> {
        let account = self.get_account()?;

        Ok(api_service::request(account, AdminListPremiumUsersRequest {})
            .map_err(|err| match err {
                ApiError::Endpoint(AdminListPremiumUsersError::NotPermissioned) => {
                    CoreError::InsufficientPermission
                }
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?
            .users)
    }
}
