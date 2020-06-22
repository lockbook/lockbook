use crate::files_db;
use crate::index_db;
use crate::ServerState;
use lockbook_core::model::api::*;

pub fn username_is_valid(username: &str) -> bool {
    username.chars().all(|x| x.is_digit(36)) && username.to_lowercase() == *username
}

pub async fn change_document_content(
    server_state: &mut ServerState,
    request: ChangeDocumentContentRequest,
) -> Result<ChangeDocumentContentResponse, ChangeDocumentContentError> {
    if !username_is_valid(&request.username) {
        return Err(ChangeDocumentContentError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(ChangeDocumentContentError::InternalError);
        }
    };

    let update_file_version_result = update_file_metadata_and_content_version(
        &transaction,
        &request.file_id,
        request.old_metadata_version as u64,
    )
    .await;
    let (old_content_version, new_version) = match update_file_version_result {
        Ok(x) => x,
        Err(update_file_metadata_and_content_version::Error::Uninterpreted(_)) => {
            println!("Internal server error! {:?}", update_file_version_result);
            return Err(ChangeDocumentContentError::InternalError);
        }
        Err(update_file_metadata_and_content_version::Error::MetadataVersionUpdate(
            update_file_metadata_version::Error::Uninterpreted(_),
        )) => {
            println!("Internal server error! {:?}", update_file_version_result);
            return Err(ChangeDocumentContentError::InternalError);
        }
        Err(update_file_metadata_and_content_version::Error::MetadataVersionUpdate(
            update_file_metadata_version::Error::VersionGeneration(_),
        )) => {
            println!("Internal server error! {:?}", update_file_version_result);
            return Err(ChangeDocumentContentError::InternalError);
        }
        Err(update_file_metadata_and_content_version::Error::MetadataVersionUpdate(
            update_file_metadata_version::Error::DocumentDoesNotExist,
        )) => return Err(ChangeDocumentContentError::DocumentNotFound),
        Err(update_file_metadata_and_content_version::Error::MetadataVersionUpdate(
            update_file_metadata_version::Error::IncorrectOldVersion(_),
        )) => return Err(ChangeDocumentContentError::EditConflict),
        Err(update_file_metadata_and_content_version::Error::MetadataVersionUpdate(
            update_file_metadata_version::Error::DocumentDeleted,
        )) => return Err(ChangeDocumentContentError::DocumentDeleted),
    };

    let create_file_result = files_db::create_file(
        &server_state.files_db_client,
        &request.file_id,
        &request.new_file_content,
        new_version,
    )
    .await;
    if create_file_result.is_err() {
        println!("Internal server error! {:?}", create_file_result);
        return Err(ChangeDocumentContentError::InternalError);
    };

    let delete_file_result = files_db::delete_file(
        &server_state.files_db_client,
        &request.file_id,
        old_content_version,
    )
    .await;
    if delete_file_result.is_err() {
        println!("Internal server error! {:?}", delete_file_result);
        return Err(ChangeDocumentContentError::InternalError);
    };

    match transaction.commit().await {
        Ok(_) => Ok(ChangeDocumentContentResponse {
            current_metadata_and_content_version: new_version,
        }),
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(ChangeDocumentContentError::InternalError)
        }
    }
}

pub async fn create_document(
    server_state: &mut ServerState,
    request: CreateDocumentRequest,
) -> Result<CreateDocumentResponse, CreateDocumentError> {
    if !username_is_valid(&request.username) {
        return Err(CreateDocumentError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(CreateDocumentError::InternalError);
        }
    };
    let get_file_details_result =
        files_db::get_file_details(&server_state.files_db_client, &request.file_id).await;
    match get_file_details_result {
        Err(files_db::get_file_details::Error::NoSuchDocument(())) => {}
        Err(_) => {
            error!("Internal server error! {:?}", get_file_details_result);
            return Err(CreateDocumentError::InternalError);
        }
        Ok(_) => return Err(CreateDocumentError::DocumentIdTaken),
    };

    let index_db_create_file_result = index_db::create_file(
        &transaction,
        &request.file_id,
        &request.username,
        &request.file_name,
        &request.file_parent,
    )
    .await;
    let new_version = match index_db_create_file_result {
        Ok(version) => version,
        Err(index_db::create_file::Error::DocumentIdTaken) => return Err(CreateDocumentError::DocumentIdTaken),
        Err(index_db::create_file::Error::DocumentPathTaken) => {
            return Err(CreateDocumentError::DocumentPathTaken)
        }
        Err(index_db::create_file::Error::Uninterpreted(_)) => {
            error!("Internal server error! {:?}", index_db_create_file_result);
            return Err(CreateDocumentError::InternalError);
        }
        Err(index_db::create_file::Error::VersionGeneration(_)) => {
            error!("Internal server error! {:?}", index_db_create_file_result);
            return Err(CreateDocumentError::InternalError);
        }
    } as u64;

    let files_db_create_file_result = files_db::create_file(
        &server_state.files_db_client,
        &request.file_id,
        &request.file_content,
        new_version,
    )
    .await;
    if files_db_create_file_result.is_err() {
        println!("Internal server error! {:?}", files_db_create_file_result);
        return Err(CreateDocumentError::InternalError);
    };

    match transaction.commit().await {
        Ok(_) => Ok(CreateDocumentResponse {
            current_metadata_and_content_version: new_version,
        }),
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(CreateDocumentError::InternalError)
        }
    }
}

pub async fn delete_document(
    server_state: &mut ServerState,
    request: DeleteDocumentRequest,
) -> Result<DeleteDocumentResponse, DeleteDocumentError> {
    if !username_is_valid(&request.username) {
        return Err(DeleteDocumentError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(DeleteDocumentError::InternalError);
        }
    };

    let index_db_delete_file_result =
        index_db::delete_file(&transaction, &request.file_id, request.old_metadata_version).await;
    let (old_content_version, new_version) = match index_db_delete_file_result {
        Ok(x) => x,
        Err(index_db::delete_file::Error::DocumentDoesNotExist) => {
            return Err(DeleteDocumentError::DocumentNotFound)
        }
        Err(index_db::delete_file::Error::DocumentDeleted) => return Err(DeleteDocumentError::DocumentDeleted),
        Err(index_db::delete_file::Error::Uninterpreted(_)) => {
            error!("Internal server error! {:?}", index_db_delete_file_result);
            return Err(DeleteDocumentError::InternalError);
        }
        Err(index_db::delete_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::Uninterpreted(_),
        )) => {
            println!("Internal server error! {:?}", index_db_delete_file_result);
            return Err(DeleteDocumentError::InternalError);
        }
        Err(index_db::delete_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::VersionGeneration(_),
        )) => {
            println!("Internal server error! {:?}", index_db_delete_file_result);
            return Err(DeleteDocumentError::InternalError);
        }
        Err(index_db::delete_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::DocumentDoesNotExist,
        )) => return Err(DeleteDocumentError::DocumentNotFound),
        Err(index_db::delete_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::IncorrectOldVersion(_),
        )) => return Err(DeleteDocumentError::EditConflict),
        Err(index_db::delete_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::DocumentDeleted,
        )) => return Err(DeleteDocumentError::DocumentDeleted),
    };

    let files_db_delete_file_result = files_db::delete_file(
        &server_state.files_db_client,
        &request.file_id,
        old_content_version,
    )
    .await;
    if files_db_delete_file_result.is_err() {
        println!("Internal server error! {:?}", files_db_delete_file_result);
        return Err(DeleteDocumentError::InternalError);
    };

    match transaction.commit().await {
        Ok(_) => Ok(DeleteDocumentResponse {
            current_metadata_and_content_version: new_version,
        }),
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(DeleteDocumentError::InternalError)
        }
    }
}

pub async fn move_document(
    server_state: &mut ServerState,
    request: MoveDocumentRequest,
) -> Result<MoveDocumentResponse, MoveDocumentError> {
    if !username_is_valid(&request.username) {
        return Err(MoveDocumentError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(MoveDocumentError::InternalError);
        }
    };

    let move_file_result = index_db::move_file(
        &transaction,
        &request.file_id,
        request.old_metadata_version,
        &request.new_file_parent,
    )
    .await;
    let result = match move_file_result {
        Ok(v) => Ok(MoveDocumentResponse {
            current_metadata_version: v,
        }),
        Err(index_db::move_file::Error::DocumentDoesNotExist) => Err(MoveDocumentError::DocumentNotFound),
        Err(index_db::move_file::Error::DocumentDeleted) => Err(MoveDocumentError::DocumentDeleted),
        Err(index_db::move_file::Error::DocumentPathTaken) => Err(MoveDocumentError::DocumentPathTaken),
        Err(index_db::move_file::Error::Uninterpreted(_)) => {
            error!("Internal server error! {:?}", move_file_result);
            Err(MoveDocumentError::InternalError)
        }
        Err(index_db::move_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::Uninterpreted(_),
        )) => {
            println!("Internal server error! {:?}", move_file_result);
            return Err(MoveDocumentError::InternalError);
        }
        Err(index_db::move_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::VersionGeneration(_),
        )) => {
            println!("Internal server error! {:?}", move_file_result);
            return Err(MoveDocumentError::InternalError);
        }
        Err(index_db::move_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::DocumentDoesNotExist,
        )) => return Err(MoveDocumentError::DocumentNotFound),
        Err(index_db::move_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::IncorrectOldVersion(_),
        )) => return Err(MoveDocumentError::EditConflict),
        Err(index_db::move_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::DocumentDeleted,
        )) => return Err(MoveDocumentError::DocumentDeleted),
    };

    match transaction.commit().await {
        Ok(_) => result,
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(MoveDocumentError::InternalError)
        }
    }
}

pub async fn rename_document(
    server_state: &mut ServerState,
    request: RenameDocumentRequest,
) -> Result<RenameDocumentResponse, RenameDocumentError> {
    if !username_is_valid(&request.username) {
        return Err(RenameDocumentError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(RenameDocumentError::InternalError);
        }
    };

    let rename_file_result = index_db::rename_file(
        &transaction,
        &request.file_id,
        request.old_metadata_version,
        &request.new_file_name,
    )
    .await;
    let result = match rename_file_result {
        Ok(v) => Ok(RenameDocumentResponse {
            current_metadata_version: v,
        }),
        Err(index_db::rename_file::Error::DocumentDoesNotExist) => Err(RenameDocumentError::DocumentNotFound),
        Err(index_db::rename_file::Error::DocumentDeleted) => Err(RenameDocumentError::DocumentDeleted),
        Err(index_db::rename_file::Error::Uninterpreted(_)) => {
            error!("Internal server error! {:?}", rename_file_result);
            Err(RenameDocumentError::InternalError)
        }
        Err(index_db::rename_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::Uninterpreted(_),
        )) => {
            println!("Internal server error! {:?}", rename_file_result);
            return Err(RenameDocumentError::InternalError);
        }
        Err(index_db::rename_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::VersionGeneration(_),
        )) => {
            println!("Internal server error! {:?}", rename_file_result);
            return Err(RenameDocumentError::InternalError);
        }
        Err(index_db::rename_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::DocumentDoesNotExist,
        )) => return Err(RenameDocumentError::DocumentNotFound),
        Err(index_db::rename_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::IncorrectOldVersion(_),
        )) => return Err(RenameDocumentError::EditConflict),
        Err(index_db::rename_file::Error::MetadataVersionUpdate(
            index_db::update_file_metadata_version::Error::DocumentDeleted,
        )) => return Err(RenameDocumentError::DocumentDeleted),
    };

    match transaction.commit().await {
        Ok(_) => result,
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(RenameDocumentError::InternalError)
        }
    }
}

pub async fn get_updates(
    server_state: &mut ServerState,
    request: GetUpdatesRequest,
) -> Result<GetUpdatesResponse, GetUpdatesError> {
    if !username_is_valid(&request.username) {
        return Err(GetUpdatesError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(GetUpdatesError::InternalError);
        }
    };
    let get_updates_result = index_db::get_updates(
        &transaction,
        &request.username,
        request.since_metadata_version as u64,
    )
    .await;
    let result = match get_updates_result {
        Ok(updates) => Ok(GetUpdatesResponse {
            file_metadata: updates,
        }),
        Err(_) => {
            error!("Internal server error! {:?}", get_updates_result);
            Err(GetUpdatesError::InternalError)
        }
    };

    match transaction.commit().await {
        Ok(_) => result,
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(GetUpdatesError::InternalError)
        }
    }
}

pub async fn new_account(
    server_state: &mut ServerState,
    request: NewAccountRequest,
) -> Result<NewAccountResponse, NewAccountError> {
    let auth = serde_json::from_str::<SignedValue>(&request.auth)
        .map_err(|_| NewAccountError::InvalidAuth)?;
    RsaImpl::verify(&request.public_key, &auth).map_err(|_| NewAccountError::InvalidPublicKey)?;
    if !username_is_valid(&request.username) {
        return Err(NewAccountError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(NewAccountError::InternalError);
        }
    };

    let new_account_result = index_db::new_account(
        &transaction,
        &request.username,
        &serde_json::to_string(&request.public_key)
            .map_err(|_| NewAccountError::InvalidPublicKey)?,
    )
    .await;
    let result = match new_account_result {
        Ok(()) => Ok(NewAccountResponse {}),
        Err(index_db::new_account::Error::UsernameTaken) => Err(NewAccountError::UsernameTaken),
        Err(index_db::new_account::Error::Uninterpreted(_)) => {
            error!("Internal server error! {:?}", new_account_result);
            Err(NewAccountError::InternalError)
        }
    };

    match transaction.commit().await {
        Ok(_) => result,
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
    let get_public_key_result = index_db::get_public_key(&transaction, &request.username).await;
    let result = match get_public_key_result {
        Ok(key) => Ok(GetPublicKeyResponse { key: key }),
        Err(index_db::get_public_key::Error::Postgres(_)) => Err(GetPublicKeyError::UserNotFound),
        Err(index_db::get_public_key::Error::SerializationError(_)) => {
            error!("Internal server error! {:?}", get_public_key_result);
            Err(GetPublicKeyError::InternalError)
        }
    };

    match transaction.commit().await {
        Ok(_) => result,
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(GetPublicKeyError::InternalError)
        }
    }
}