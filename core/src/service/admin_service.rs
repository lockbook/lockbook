use crate::service::api_service;
use crate::service::api_service::ApiError;
use crate::{core_err_unexpected, CoreError, RequestContext};
use lockbook_models::api::{
    AdminDeleteAccountError, AdminDeleteAccountRequest, FeatureFlag, GetFeatureFlagsStateError,
    GetFeatureFlagsStateRequest, ToggleFeatureFlagError, ToggleFeatureFlagRequest,
};
use std::collections::HashMap;

impl RequestContext<'_, '_> {
    pub fn delete_account(&self, username: &str) -> Result<(), CoreError> {
        let account = self.get_account()?;

        api_service::request(&account, AdminDeleteAccountRequest { username: username.to_string() })
            .map_err(|err| match err {
                ApiError::Endpoint(AdminDeleteAccountError::UsernameNotFound) => {
                    CoreError::UsernameNotFound
                }
                ApiError::Endpoint(AdminDeleteAccountError::Unauthorized) => {
                    CoreError::Unauthorized
                }
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })
    }

    pub fn get_feature_flags_state(&self) -> Result<HashMap<FeatureFlag, bool>, CoreError> {
        let account = self.get_account()?;

        Ok(api_service::request(&account, GetFeatureFlagsStateRequest {})
            .map_err(|err| match err {
                ApiError::Endpoint(GetFeatureFlagsStateError::Unauthorized) => {
                    CoreError::Unauthorized
                }
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            })?
            .states)
    }

    pub fn toggle_feature_flag(&self, feature: FeatureFlag, enable: bool) -> Result<(), CoreError> {
        let account = self.get_account()?;

        api_service::request(&account, ToggleFeatureFlagRequest { feature, enable }).map_err(
            |err| match err {
                ApiError::Endpoint(ToggleFeatureFlagError::Unauthorized) => CoreError::Unauthorized,
                ApiError::SendFailed(_) => CoreError::ServerUnreachable,
                ApiError::ClientUpdateRequired => CoreError::ClientUpdateRequired,
                _ => core_err_unexpected(err),
            },
        )
    }
}
