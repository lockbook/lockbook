use crate::model::errors::core_err_unexpected;
use crate::service::api_service::ApiError;
use crate::{CoreError, CoreResult, CoreState, Requester};
use lockbook_shared::account::Username;
use lockbook_shared::api::*;
use uuid::Uuid;

impl<Client: Requester> CoreState<Client> {
    pub(crate) fn disappear_account(&self, username: &str) -> CoreResult<()> {
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

    pub(crate) fn disappear_file(&self, id: Uuid) -> CoreResult<()> {
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

    pub(crate) fn list_users(&self, filter: Option<AccountFilter>) -> CoreResult<Vec<Username>> {
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

    pub(crate) fn get_account_info(
        &self, identifier: AccountIdentifier,
    ) -> CoreResult<AccountInfo> {
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

    pub(crate) fn validate_account(&self, username: &str) -> CoreResult<AdminValidateAccount> {
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

    pub(crate) fn validate_server(&self) -> CoreResult<AdminValidateServer> {
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

    pub(crate) fn file_info(&self, id: Uuid) -> CoreResult<AdminFileInfoResponse> {
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

    pub(crate) fn rebuild_index(&self, index: ServerIndex) -> CoreResult<()> {
        let account = self.get_account()?;
        self.client
            .request(account, AdminRebuildIndexRequest { index })
            .map_err(|err| match err {
                ApiError::Endpoint(AdminRebuildIndexError::NotPermissioned) => {
                    CoreError::InsufficientPermission
                }
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })
    }

    pub(crate) fn set_user_tier(
        &self, username: &str, info: AdminSetUserTierInfo,
    ) -> CoreResult<()> {
        let account = self.get_account()?;
        self.client
            .request(account, AdminSetUserTierRequest { username: username.to_string(), info })
            .map_err(|err| match err {
                ApiError::Endpoint(AdminSetUserTierError::NotPermissioned) => {
                    CoreError::InsufficientPermission
                }
                ApiError::Endpoint(AdminSetUserTierError::UserNotFound) => {
                    CoreError::UsernameNotFound
                }
                ApiError::Endpoint(AdminSetUserTierError::ExistingRequestPending) => {
                    CoreError::ExistingRequestPending
                }
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        Ok(())
    }
}
