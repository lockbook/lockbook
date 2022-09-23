use crate::service::api_service;
use crate::service::api_service::ApiError;
use crate::{core_err_unexpected, CoreError, CoreResult, RequestContext};
use lockbook_shared::account::Username;
use lockbook_shared::api::{
    AccountFilter, AccountIdentifier, AccountInfo, AdminDisappearAccountError,
    AdminDisappearAccountRequest, AdminDisappearFileError, AdminDisappearFileRequest,
    AdminGetAccountInfoError, AdminGetAccountInfoRequest, AdminListUsersError,
    AdminListUsersRequest, AdminServerValidateError, AdminServerValidateRequest,
    AdminServerValidateResponse,
};
use uuid::Uuid;

impl RequestContext<'_, '_> {
    pub fn disappear_account(&self, username: &str) -> CoreResult<()> {
        let account = self.get_account()?;

        api_service::request(
            account,
            AdminDisappearAccountRequest { username: username.to_string() },
        )
        .map_err(|err| match err {
            ApiError::Endpoint(AdminDisappearAccountError::UserNotFound) => {
                CoreError::UsernameNotFound
            }
            ApiError::Endpoint(AdminDisappearAccountError::NotPermissioned) => {
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

    pub fn list_users(&self, filter: Option<AccountFilter>) -> CoreResult<Vec<Username>> {
        let account = self.get_account()?;

        Ok(api_service::request(account, AdminListUsersRequest { filter })
            .map_err(|err| match err {
                ApiError::Endpoint(AdminListUsersError::NotPermissioned) => {
                    CoreError::InsufficientPermission
                }
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?
            .users)
    }

    pub fn get_account_info(&self, identifier: AccountIdentifier) -> CoreResult<AccountInfo> {
        let account = self.get_account()?;

        Ok(api_service::request(account, AdminGetAccountInfoRequest { identifier })
            .map_err(|err| match err {
                ApiError::Endpoint(AdminGetAccountInfoError::NotPermissioned) => {
                    CoreError::InsufficientPermission
                }
                ApiError::Endpoint(AdminGetAccountInfoError::UserNotFound) => {
                    CoreError::UsernameNotFound
                }
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?
            .account)
    }

    pub fn server_validate(&self, username: &str) -> CoreResult<AdminServerValidateResponse> {
        let account = self.get_account()?;
        api_service::request(account, AdminServerValidateRequest { username: username.to_string() })
            .map_err(|err| match err {
                ApiError::Endpoint(AdminServerValidateError::NotPermissioned) => {
                    CoreError::InsufficientPermission
                }
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })
    }
}
