use crate::service::api_service;
use crate::service::api_service::ApiError;
use crate::{core_err_unexpected, CoreError, RequestContext};
use lockbook_models::api::{
    AdminDeleteAccountError, AdminDeleteAccountRequest, FeatureFlagError,
    GetFeatureFlagsStateRequest, ToggleFeatureFlagRequest,
};
use lockbook_models::feature_flag::{FeatureFlag, FeatureFlags};

impl RequestContext<'_, '_> {
    pub fn delete_account(&self, username: &str) -> Result<(), CoreError> {
        let account = self.get_account()?;

        api_service::request(&account, AdminDeleteAccountRequest { username: username.to_string() })
            .map_err(|err| match err {
                ApiError::Endpoint(AdminDeleteAccountError::UserNotFound) => {
                    CoreError::UsernameNotFound
                }
                ApiError::Endpoint(AdminDeleteAccountError::NotPermissioned) => {
                    CoreError::NotPermissioned
                }
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })
    }

    pub fn get_feature_flags_state(&self) -> Result<FeatureFlags, CoreError> {
        let account = self.get_account()?;

        Ok(api_service::request(&account, GetFeatureFlagsStateRequest {})
            .map_err(|err| match err {
                ApiError::Endpoint(FeatureFlagError::NotPermissioned) => CoreError::NotPermissioned,
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?
            .feature_flags)
    }

    pub fn toggle_feature_flag(
        &self, feature_flag: FeatureFlag, enable: bool,
    ) -> Result<(), CoreError> {
        let account = self.get_account()?;

        api_service::request(&account, ToggleFeatureFlagRequest { feature_flag, enable }).map_err(
            |err| match err {
                ApiError::Endpoint(FeatureFlagError::NotPermissioned) => CoreError::NotPermissioned,
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            },
        )
    }
}
