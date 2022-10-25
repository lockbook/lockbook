use crate::service::api_service::ApiError;
use crate::{core_err_unexpected, CoreError, CoreResult, RequestContext, Requester};
use lockbook_shared::account::Username;
use lockbook_shared::api::*;
use uuid::Uuid;

impl<Client: Requester> RequestContext<'_, '_, Client> {
    pub fn disappear_account(&self, username: &str) -> CoreResult<()> {
        let account = self.get_account()?;

        self.client
            .request(account, AdminDisappearAccountRequest { username: username.to_string() })
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
        self.client
            .request(account, AdminDisappearFileRequest { id })
            .map_err(|err| match err {
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

        Ok(self
            .client
            .request(account, AdminListUsersRequest { filter })
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

        Ok(self
            .client
            .request(account, AdminGetAccountInfoRequest { identifier })
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

    pub fn validate_account(&self, username: &str) -> CoreResult<AdminValidateAccount> {
        let account = self.get_account()?;
        self.client
            .request(account, AdminValidateAccountRequest { username: username.to_string() })
            .map_err(|err| match err {
                ApiError::Endpoint(AdminValidateAccountError::NotPermissioned) => {
                    CoreError::InsufficientPermission
                }
                ApiError::Endpoint(AdminValidateAccountError::UserNotFound) => {
                    CoreError::UsernameNotFound
                }
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })
    }

    pub fn validate_server(&self) -> CoreResult<AdminValidateServer> {
        let account = self.get_account()?;
        self.client
            .request(account, AdminValidateServerRequest {})
            .map_err(|err| match err {
                ApiError::Endpoint(AdminValidateServerError::NotPermissioned) => {
                    CoreError::InsufficientPermission
                }
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })
    }

    pub fn file_info(&self, id: Uuid) -> CoreResult<AdminFileInfoResponse> {
        let account = self.get_account()?;
        self.client
            .request(account, AdminFileInfoRequest { id })
            .map_err(|err| match err {
                ApiError::Endpoint(AdminFileInfoError::FileNonexistent) => {
                    CoreError::FileNonexistent
                }
                ApiError::Endpoint(AdminFileInfoError::NotPermissioned) => {
                    CoreError::InsufficientPermission
                }
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })
    }
}
