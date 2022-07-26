use crate::{account_service, ClientError, RequestContext, ServerError, ServerV1};
use lockbook_models::api::{
    FeatureFlag, GetFeatureFlagsStateError, GetFeatureFlagsStateRequest,
    GetFeatureFlagsStateResponse, ToggleFeatureFlagError, ToggleFeatureFlagRequest,
};
use std::fmt::Debug;

pub const FEATURE_FLAGS: [FeatureFlag; 1] = [FeatureFlag::NewAccounts];

pub fn initialize_flags(db: &ServerV1) {
    for flag in FEATURE_FLAGS {
        if !db.feature_flags.exists(&flag).unwrap() {
            db.feature_flags.insert(flag, true).unwrap();
        }
    }
}

pub fn is_new_accounts_enabled<T: Debug>(db: &ServerV1) -> Result<bool, ServerError<T>> {
    db.feature_flags
        .get(&FeatureFlag::NewAccounts)?
        .ok_or_else(|| internal!("No new accounts feature flag is defined!"))
}

pub async fn toggle_feature_flag(
    context: RequestContext<'_, ToggleFeatureFlagRequest>,
) -> Result<(), ServerError<ToggleFeatureFlagError>> {
    let (request, db) = (&context.request, &context.server_state.index_db);

    if !account_service::is_user_authorized(
        &context.public_key,
        &context.server_state.config.admin.admins,
        db,
    )? {
        return Err(ClientError(ToggleFeatureFlagError::Unauthorized));
    }

    db.feature_flags
        .insert(request.feature.clone(), request.enable)?;

    Ok(())
}

pub async fn get_feature_flags_state(
    context: RequestContext<'_, GetFeatureFlagsStateRequest>,
) -> Result<GetFeatureFlagsStateResponse, ServerError<GetFeatureFlagsStateError>> {
    let db = &context.server_state.index_db;

    if !account_service::is_user_authorized(
        &context.public_key,
        &context.server_state.config.admin.admins,
        db,
    )? {
        return Err(ClientError(GetFeatureFlagsStateError::Unauthorized));
    }

    Ok(GetFeatureFlagsStateResponse { states: db.feature_flags.get_all()? })
}
