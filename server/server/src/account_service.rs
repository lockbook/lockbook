use crate::utils::username_is_valid;
use crate::{file_index_repo, RequestContext};

use lockbook_models::api::{
    GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse, GetUsageError, GetUsageRequest,
    GetUsageResponse, NewAccountError, NewAccountRequest, NewAccountResponse,
};
use lockbook_models::file_metadata::FileType;

pub async fn new_account(
    context: RequestContext<'_, NewAccountRequest>,
) -> Result<NewAccountResponse, Result<NewAccountError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
    if !username_is_valid(&request.username) {
        return Err(Ok(NewAccountError::InvalidUsername));
    }

    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let new_account_result =
        file_index_repo::new_account(&mut transaction, &request.username, &request.public_key)
            .await;
    new_account_result.map_err(|e| match e {
        file_index_repo::NewAccountError::UsernameTaken => Ok(NewAccountError::UsernameTaken),
        _ => Err(format!("Cannot create account in Postgres: {:?}", e)),
    })?;

    let create_folder_result = file_index_repo::create_file(
        &mut transaction,
        request.folder_id,
        request.folder_id,
        FileType::Folder,
        &request.folder_name,
        &context.public_key,
        &request.parent_access_key,
        None,
    )
    .await;
    let new_version = create_folder_result.map_err(|e| match e {
        file_index_repo::CreateFileError::IdTaken => Ok(NewAccountError::FileIdTaken),
        _ => Err(format!(
            "Cannot create account root folder in Postgres: {:?}",
            e
        )),
    })?;
    let new_user_access_key_result = file_index_repo::create_user_access_key(
        &mut transaction,
        &request.public_key,
        request.folder_id,
        &request.user_access_key,
    )
    .await;
    new_user_access_key_result.map_err(|e| {
        Err(format!(
            "Cannot create access keys for user in Postgres: {:?}",
            e
        ))
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(NewAccountResponse {
            folder_metadata_version: new_version,
        }),
        Err(sqlx::Error::Database(db_err)) => match db_err.constraint() {
            Some("uk_name") => Err(Ok(NewAccountError::UsernameTaken)),
            _ => Err(Err(format!(
                "Cannot commit transaction due to constraint violation: {:?}",
                db_err
            ))),
        },
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn get_public_key(
    context: RequestContext<'_, GetPublicKeyRequest>,
) -> Result<GetPublicKeyResponse, Result<GetPublicKeyError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };
    let result = file_index_repo::get_public_key(&mut transaction, &request.username).await;
    let key = result.map_err(|e| match e {
        file_index_repo::PublicKeyError::UserNotFound => Ok(GetPublicKeyError::UserNotFound),
        _ => Err(format!("Cannot get public key from Postgres: {:?}", e)),
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(GetPublicKeyResponse { key: key }),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn get_usage(
    context: RequestContext<'_, GetUsageRequest>,
) -> Result<GetUsageResponse, Result<GetUsageError, String>> {
    let (_, server_state) = (&context.request, context.server_state);
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:#?}", e)));
        }
    };

    let usages = file_index_repo::get_file_usages(&mut transaction, &context.public_key)
        .await
        .map_err(|e| Err(format!("Usage calculation error: {:#?}", e)))?;

    let cap = file_index_repo::get_account_data_cap(&mut transaction, &context.public_key)
        .await
        .map_err(|e| Err(format!("Data cap calculation error: {:#?}", e)))?;

    Ok(GetUsageResponse { usages, cap })
}
