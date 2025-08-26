use crate::Lb;
use crate::io::network::ApiError;
use crate::model::account::Username;
use crate::model::api::*;
use crate::LbServer;
use crate::model::errors::{LbErrKind, LbResult, core_err_unexpected};
use uuid::Uuid;

impl LbServer {
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn disappear_account(&self, username: &str) -> LbResult<()> {
        let account = self.get_account()?;

        self.client
            .request(account, AdminDisappearAccountRequest { username: username.to_string() })
            .await
            .map_err(|err| {
                match err {
                    ApiError::Endpoint(AdminDisappearAccountError::UserNotFound) => {
                        LbErrKind::UsernameNotFound
                    }
                    ApiError::Endpoint(AdminDisappearAccountError::NotPermissioned) => {
                        LbErrKind::InsufficientPermission
                    }
                    ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                    ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                    _ => core_err_unexpected(err),
                }
                .into()
            })
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn disappear_file(&self, id: Uuid) -> LbResult<()> {
        let account = self.get_account()?;
        self.client
            .request(account, AdminDisappearFileRequest { id })
            .await
            .map_err(|err| {
                match err {
                    ApiError::Endpoint(AdminDisappearFileError::FileNonexistent) => {
                        LbErrKind::FileNonexistent
                    }
                    ApiError::Endpoint(AdminDisappearFileError::NotPermissioned) => {
                        LbErrKind::InsufficientPermission
                    }
                    ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                    ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                    _ => core_err_unexpected(err),
                }
                .into()
            })
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn list_users(&self, filter: Option<AccountFilter>) -> LbResult<Vec<Username>> {
        let account = self.get_account()?;

        Ok(self
            .client
            .request(account, AdminListUsersRequest { filter })
            .await
            .map_err(|err| match err {
                ApiError::Endpoint(AdminListUsersError::NotPermissioned) => {
                    LbErrKind::InsufficientPermission
                }
                ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?
            .users)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn get_account_info(&self, identifier: AccountIdentifier) -> LbResult<AccountInfo> {
        let account = self.get_account()?;

        Ok(self
            .client
            .request(account, AdminGetAccountInfoRequest { identifier })
            .await
            .map_err(|err| match err {
                ApiError::Endpoint(AdminGetAccountInfoError::NotPermissioned) => {
                    LbErrKind::InsufficientPermission
                }
                ApiError::Endpoint(AdminGetAccountInfoError::UserNotFound) => {
                    LbErrKind::UsernameNotFound
                }
                ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?
            .account)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn validate_account(&self, username: &str) -> LbResult<AdminValidateAccount> {
        let account = self.get_account()?;
        self.client
            .request(account, AdminValidateAccountRequest { username: username.to_string() })
            .await
            .map_err(|err| {
                match err {
                    ApiError::Endpoint(AdminValidateAccountError::NotPermissioned) => {
                        LbErrKind::InsufficientPermission
                    }
                    ApiError::Endpoint(AdminValidateAccountError::UserNotFound) => {
                        LbErrKind::UsernameNotFound
                    }
                    ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                    ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                    _ => core_err_unexpected(err),
                }
                .into()
            })
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn validate_server(&self) -> LbResult<AdminValidateServer> {
        let account = self.get_account()?;
        self.client
            .request(account, AdminValidateServerRequest {})
            .await
            .map_err(|err| {
                match err {
                    ApiError::Endpoint(AdminValidateServerError::NotPermissioned) => {
                        LbErrKind::InsufficientPermission
                    }
                    ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                    ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                    _ => core_err_unexpected(err),
                }
                .into()
            })
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn file_info(&self, id: Uuid) -> LbResult<AdminFileInfoResponse> {
        let account = self.get_account()?;
        self.client
            .request(account, AdminFileInfoRequest { id })
            .await
            .map_err(|err| {
                match err {
                    ApiError::Endpoint(AdminFileInfoError::FileNonexistent) => {
                        LbErrKind::FileNonexistent
                    }
                    ApiError::Endpoint(AdminFileInfoError::NotPermissioned) => {
                        LbErrKind::InsufficientPermission
                    }
                    ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                    ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                    _ => core_err_unexpected(err),
                }
                .into()
            })
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn rebuild_index(&self, index: ServerIndex) -> LbResult<()> {
        let account = self.get_account()?;
        self.client
            .request(account, AdminRebuildIndexRequest { index })
            .await
            .map_err(|err| {
                match err {
                    ApiError::Endpoint(AdminRebuildIndexError::NotPermissioned) => {
                        LbErrKind::InsufficientPermission
                    }
                    ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                    ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                    _ => core_err_unexpected(err),
                }
                .into()
            })
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn set_user_tier(&self, username: &str, info: AdminSetUserTierInfo) -> LbResult<()> {
        let account = self.get_account()?;
        self.client
            .request(account, AdminSetUserTierRequest { username: username.to_string(), info })
            .await
            .map_err(|err| match err {
                ApiError::Endpoint(AdminSetUserTierError::NotPermissioned) => {
                    LbErrKind::InsufficientPermission
                }
                ApiError::Endpoint(AdminSetUserTierError::UserNotFound) => {
                    LbErrKind::UsernameNotFound
                }
                ApiError::Endpoint(AdminSetUserTierError::ExistingRequestPending) => {
                    LbErrKind::ExistingRequestPending
                }
                ApiError::SendFailed(_) => LbErrKind::ServerUnreachable,
                ApiError::ClientUpdateRequired => LbErrKind::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?;

        Ok(())
    }
}
