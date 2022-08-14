use crate::schema::OneKey;
use crate::{account_service, ClientError, RequestContext, ServerError, ServerV1};
use lockbook_shared::api::{
    FeatureFlagError, GetFeatureFlagsStateRequest, GetFeatureFlagsStateResponse,
    ToggleFeatureFlagRequest,
};
use lockbook_shared::feature_flag::{FeatureFlag, FeatureFlags};
use std::fmt::Debug;

pub fn initialize_flags(db: &ServerV1) {
    if !db.feature_flags.exists(&OneKey {}).unwrap() {
        db.feature_flags
            .insert(OneKey {}, FeatureFlags::default())
            .unwrap();
    }
}

pub fn is_new_accounts_enabled<T: Debug>(db: &ServerV1) -> Result<bool, ServerError<T>> {
    Ok(db
        .feature_flags
        .get(&OneKey {})?
        .ok_or_else(|| internal!("No feature flags defined."))?
        .new_accounts)
}

pub async fn toggle_feature_flag(
    context: RequestContext<'_, ToggleFeatureFlagRequest>,
) -> Result<(), ServerError<FeatureFlagError>> {
    let (request, db) = (&context.request, &context.server_state.index_db);

    if !account_service::is_admin(
        db,
        &context.public_key,
        &context.server_state.config.admin.admins,
    )? {
        return Err(ClientError(FeatureFlagError::NotPermissioned));
    }

    let mut feature_flags = db
        .feature_flags
        .get(&OneKey {})?
        .ok_or_else(|| internal!("No feature flags defined."))?;

    match request.feature_flag {
        FeatureFlag::NewAccounts => feature_flags.new_accounts = request.enable,
    };

    db.feature_flags.insert(OneKey {}, feature_flags)?;

    Ok(())
}

pub async fn get_feature_flags_state(
    context: RequestContext<'_, GetFeatureFlagsStateRequest>,
) -> Result<GetFeatureFlagsStateResponse, ServerError<FeatureFlagError>> {
    let db = &context.server_state.index_db;

    if !account_service::is_admin(
        db,
        &context.public_key,
        &context.server_state.config.admin.admins,
    )? {
        return Err(ClientError(FeatureFlagError::NotPermissioned));
    }

    let feature_flags = db
        .feature_flags
        .get(&OneKey {})?
        .ok_or_else(|| internal!("No feature flags defined."))?;

    Ok(GetFeatureFlagsStateResponse { feature_flags })
}
