use crate::utils::username_is_valid;
use crate::{file_index_repo, RequestContext};

use lockbook_models::api::{
    GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse, GetUsageError, GetUsageRequest,
    GetUsageResponse, NewAccountError, NewAccountRequest, NewAccountResponse,
};
use lockbook_models::file_metadata::FileType;

pub async fn new_account(
    context: &mut RequestContext<'_, NewAccountRequest>,
) -> Result<NewAccountResponse, Option<NewAccountError>> {
    let request = &context.request;
    let server_state = &mut context.server_state;
    if !username_is_valid(&request.username) {
        return Err(Some(NewAccountError::InvalidUsername));
    }

    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(None);
        }
    };

    let new_account_result =
        file_index_repo::new_account(&mut transaction, &request.username, &request.public_key)
            .await;
    new_account_result.map_err(|e| match e {
        file_index_repo::NewAccountError::UsernameTaken => Some(NewAccountError::UsernameTaken),
        _ => {
            error!(
                "Internal server error! Cannot create account in Postgres: {:?}",
                e
            );
            None
        }
    })?;

    let create_folder_result = file_index_repo::create_file(
        &mut transaction,
        request.folder_id,
        request.folder_id,
        FileType::Folder,
        &request.username,
        &context.public_key,
        &request.parent_access_key,
        None,
    )
    .await;
    let new_version = create_folder_result.map_err(|e| match e {
        file_index_repo::CreateFileError::IdTaken => Some(NewAccountError::FileIdTaken),
        _ => {
            error!(
                "Internal server error! Cannot create account root folder in Postgres: {:?}",
                e
            );
            None
        }
    })?;
    let new_user_access_key_result = file_index_repo::create_user_access_key(
        &mut transaction,
        &request.username,
        request.folder_id,
        &request.user_access_key,
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
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(None);
        }
    };
    let result = file_index_repo::get_public_key(&mut transaction, &request.username).await;
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
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:#?}", e);
            return Err(None);
        }
    };

    let usages = file_index_repo::get_file_usages(&mut transaction, &context.public_key)
        .await
        .map_err(|e| {
            error!("Usage calculation error: {:#?}", e);
            None
        })?;

    let cap = file_index_repo::get_account_data_cap(&mut transaction, &context.public_key)
        .await
        .map_err(|e| {
            error!("Data cap calculation error: {:#?}", e);
            None
        })?;

    Ok(GetUsageResponse { usages, cap })
}
