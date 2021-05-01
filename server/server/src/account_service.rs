use crate::file_index_repo::create_free_account_tier_row;
use crate::utils::username_is_valid;
use crate::{file_index_repo, usage_repo, RequestContext};
use chrono::FixedOffset;
use lockbook_models::api::{
    GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse, GetUsageError, GetUsageRequest,
    GetUsageResponse, NewAccountError, NewAccountRequest, NewAccountResponse,
};
use lockbook_models::file_metadata::FileType;
use std::ops::Add;

pub async fn new_account(
    context: &mut RequestContext<'_, NewAccountRequest>,
) -> Result<NewAccountResponse, Option<NewAccountError>> {
    let request = &context.request;
    let server_state = &mut context.server_state;
    if !username_is_valid(&request.username) {
        return Err(Some(NewAccountError::InvalidUsername));
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(None);
        }
    };

    let tier_id = create_free_account_tier_row(&transaction)
        .await
        .map_err(|e| {
            error!(
                "Internal server error! Could not create tier row for new account {:?}",
                e
            );
            None
        })?;

    let new_account_result = file_index_repo::new_account(
        &transaction,
        &request.username,
        &serde_json::to_string(&request.public_key)
            .map_err(|_| Some(NewAccountError::InvalidPublicKey))?,
        tier_id,
    )
    .await;
    new_account_result.map_err(|e| match e {
        file_index_repo::AccountError::UsernameTaken => Some(NewAccountError::UsernameTaken),
        _ => {
            error!(
                "Internal server error! Cannot create account in Postgres: {:?}",
                e
            );
            None
        }
    })?;

    let create_folder_result = file_index_repo::create_file(
        &transaction,
        request.folder_id,
        request.folder_id,
        FileType::Folder,
        &request.username,
        &context.public_key,
        &request.parent_access_key,
    )
    .await;
    let new_version = create_folder_result.map_err(|e| match e {
        file_index_repo::FileError::IdTaken => Some(NewAccountError::FileIdTaken),
        _ => {
            error!(
                "Internal server error! Cannot create account root folder in Postgres: {:?}",
                e
            );
            None
        }
    })?;
    let new_user_access_key_result = file_index_repo::create_user_access_key(
        &transaction,
        &request.username,
        request.folder_id,
        &serde_json::to_string(&request.user_access_key)
            .map_err(|_| Some(NewAccountError::InvalidUserAccessKey))?,
    )
    .await;
    new_user_access_key_result.map_err(|e| {
        error!(
            "Internal server error! Cannot create access keys for user in Postgres: {:?}",
            e
        );
        None
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(NewAccountResponse {
            folder_metadata_version: new_version,
        }),
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(None)
        }
    }
}

pub async fn get_public_key(
    context: &mut RequestContext<'_, GetPublicKeyRequest>,
) -> Result<GetPublicKeyResponse, Option<GetPublicKeyError>> {
    let request = &context.request;
    let server_state = &mut context.server_state;
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(None);
        }
    };
    let result = file_index_repo::get_public_key(&transaction, &request.username).await;
    let key = result.map_err(|e| match e {
        file_index_repo::PublicKeyError::UserNotFound => Some(GetPublicKeyError::UserNotFound),
        _ => {
            error!(
                "Internal server error! Cannot get public key from Postgres: {:?}",
                e
            );
            None
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(GetPublicKeyResponse { key: key }),
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(None)
        }
    }
}

pub async fn get_usage(
    context: &mut RequestContext<'_, GetUsageRequest>,
) -> Result<GetUsageResponse, Option<GetUsageError>> {
    let server_state = &mut context.server_state;
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:#?}", e);
            return Err(None);
        }
    };

    let timestamp = chrono::Local::now().naive_utc();

    let res = usage_repo::calculate(
        &transaction,
        &context.public_key,
        timestamp,
        timestamp.add(FixedOffset::east(1)),
    )
    .await
    .map_err(|e| {
        error!("Usage calculation error: {:#?}", e);
        None
    })?;

    Ok(GetUsageResponse { usages: res })
}
