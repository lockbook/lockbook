use crate::utils::{username_is_valid, version_is_supported};
use crate::{file_index_repo, usage_service, ServerState};
use chrono::FixedOffset;
use lockbook_models::api::{
    GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse, GetUsageError, GetUsageRequest,
    GetUsageResponse, NewAccountError, NewAccountRequest, NewAccountResponse,
};
use lockbook_models::file_metadata::FileType;
use std::ops::Add;

pub async fn new_account(
    server_state: &mut ServerState,
    request: NewAccountRequest,
) -> Result<NewAccountResponse, NewAccountError> {
    if !version_is_supported(&request.client_version) {
        return Err(NewAccountError::ClientUpdateRequired);
    }

    // let auth = serde_json::from_str::<SignedValue>(&request.auth)
    //     .map_err(|_| NewAccountError::InvalidAuth)?;
    // RsaImpl::verify(&request.public_key, &auth).map_err(|_| NewAccountError::InvalidPublicKey)?;
    if !username_is_valid(&request.username) {
        debug!("{} is not a valid username", request.username);
        return Err(NewAccountError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(NewAccountError::InternalError);
        }
    };

    let new_account_result = file_index_repo::new_account(
        &transaction,
        &request.username,
        &serde_json::to_string(&request.public_key)
            .map_err(|_| NewAccountError::InvalidPublicKey)?,
    )
    .await;
    new_account_result.map_err(|e| match e {
        file_index_repo::AccountError::UsernameTaken => NewAccountError::UsernameTaken,
        _ => {
            error!(
                "Internal server error! Cannot create account in Postgres: {:?}",
                e
            );
            NewAccountError::InternalError
        }
    })?;

    let create_folder_result = file_index_repo::create_file(
        &transaction,
        request.folder_id,
        request.folder_id,
        FileType::Folder,
        &request.username,
        &request.username,
        &request.signature,
        &request.parent_access_key,
    )
    .await;
    let new_version = create_folder_result.map_err(|e| match e {
        file_index_repo::FileError::IdTaken => NewAccountError::FileIdTaken,
        _ => {
            error!(
                "Internal server error! Cannot create account root folder in Postgres: {:?}",
                e
            );
            NewAccountError::InternalError
        }
    })?;
    let new_user_access_key_result = file_index_repo::create_user_access_key(
        &transaction,
        &request.username,
        request.folder_id,
        &serde_json::to_string(&request.user_access_key)
            .map_err(|_| NewAccountError::InvalidUserAccessKey)?,
    )
    .await;
    new_user_access_key_result.map_err(|e| {
        error!(
            "Internal server error! Cannot create access keys for user in Postgres: {:?}",
            e
        );
        NewAccountError::InternalError
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(NewAccountResponse {
            folder_metadata_version: new_version,
        }),
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(NewAccountError::InternalError)
        }
    }
}

pub async fn get_public_key(
    server_state: &mut ServerState,
    request: GetPublicKeyRequest,
) -> Result<GetPublicKeyResponse, GetPublicKeyError> {
    if !version_is_supported(&request.client_version) {
        return Err(GetPublicKeyError::ClientUpdateRequired);
    }

    if !username_is_valid(&request.username) {
        return Err(GetPublicKeyError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(GetPublicKeyError::InternalError);
        }
    };
    let result = file_index_repo::get_public_key(&transaction, &request.username).await;
    let key = result.map_err(|e| match e {
        file_index_repo::PublicKeyError::UserNotFound => GetPublicKeyError::UserNotFound,
        _ => {
            error!(
                "Internal server error! Cannot get public key from Postgres: {:?}",
                e
            );
            GetPublicKeyError::InternalError
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(GetPublicKeyResponse { key: key }),
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(GetPublicKeyError::InternalError)
        }
    }
}

pub async fn calculate_usage(
    server_state: &mut ServerState,
    request: GetUsageRequest,
) -> Result<GetUsageResponse, GetUsageError> {
    if !version_is_supported(&request.client_version) {
        return Err(GetUsageError::ClientUpdateRequired);
    }

    if !username_is_valid(&request.username) {
        debug!("{} is not a valid username", request.username);
        return Err(GetUsageError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:#?}", e);
            return Err(GetUsageError::InternalError);
        }
    };

    let timestamp = chrono::Local::now().naive_utc();

    let res = usage_service::calculate(
        &transaction,
        &request.username,
        timestamp,
        timestamp.add(FixedOffset::east(1)),
    )
    .await
    .map_err(|e| {
        error!("Usage calculation error: {:#?}", e);
        GetUsageError::InternalError
    })?;

    Ok(GetUsageResponse { usages: res })
}
