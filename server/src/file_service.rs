use crate::files_db;
use crate::index_db;
use crate::ServerState;
use lockbook_core::model::api::*;
use lockbook_core::model::client_file_metadata::FileType;

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

    let result = index_db::change_document_content_version(
        &transaction,
        request.id,
        request.old_metadata_version,
    )
    .await;
    let (old_content_version, new_version) = result.map_err(|e| match e {
        index_db::FileError::DoesNotExist => ChangeDocumentContentError::DocumentNotFound,
        index_db::FileError::IncorrectOldVersion => ChangeDocumentContentError::EditConflict,
        index_db::FileError::Deleted => ChangeDocumentContentError::DocumentDeleted,
        _ => {
            println!("Internal server error! {:?}", e);
            ChangeDocumentContentError::InternalError
        }
    })?;

    let create_result = files_db::create(
        &server_state.files_db_client,
        request.id,
        new_version,
        &request.new_content,
    )
    .await;
    if create_result.is_err() {
        println!("Internal server error! {:?}", create_result);
        return Err(ChangeDocumentContentError::InternalError);
    };

    let delete_result = files_db::delete(
        &server_state.files_db_client,
        request.id,
        old_content_version,
    )
    .await;
    if delete_result.is_err() {
        println!("Internal server error! {:?}", delete_result);
        return Err(ChangeDocumentContentError::InternalError);
    };

    match transaction.commit().await {
        Ok(()) => Ok(ChangeDocumentContentResponse {
            new_metadata_and_content_version: new_version,
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

    let index_result = index_db::create_file(
        &transaction,
        request.id,
        request.parent,
        FileType::Document,
        &request.name,
        &request.username,
        &request.signature,
        &request.parent_access_key,
    )
    .await;
    let new_version = index_result.map_err(|e| match e {
        index_db::FileError::IdTaken => CreateDocumentError::FileIdTaken,
        index_db::FileError::PathTaken => CreateDocumentError::DocumentPathTaken,
        _ => {
            println!("Internal server error! {:?}", e);
            CreateDocumentError::InternalError
        }
    })?;

    let files_result = files_db::create(
        &server_state.files_db_client,
        request.id,
        new_version,
        &request.content,
    )
    .await;
    if files_result.is_err() {
        println!("Internal server error! {:?}", files_result);
        return Err(CreateDocumentError::InternalError);
    };

    match transaction.commit().await {
        Ok(()) => Ok(CreateDocumentResponse {
            new_metadata_and_content_version: new_version,
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

    let index_result =
        index_db::delete_file(&transaction, request.id, request.old_metadata_version, FileType::Document).await;
    let (old_content_version, new_version) = index_result.map_err(|e| match e {
        index_db::FileError::DoesNotExist => DeleteDocumentError::DocumentNotFound,
        index_db::FileError::IncorrectOldVersion => DeleteDocumentError::EditConflict,
        index_db::FileError::Deleted => DeleteDocumentError::DocumentDeleted,
        _ => {
            println!("Internal server error! {:?}", e);
            DeleteDocumentError::InternalError
        }
    })?;

    let files_result = files_db::delete(
        &server_state.files_db_client,
        request.id,
        old_content_version,
    )
    .await;
    if files_result.is_err() {
        println!("Internal server error! {:?}", files_result);
        return Err(DeleteDocumentError::InternalError);
    };

    match transaction.commit().await {
        Ok(()) => Ok(DeleteDocumentResponse {
            new_metadata_and_content_version: new_version,
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

    let result = index_db::move_file(
        &transaction,
        request.id,
        request.old_metadata_version,
        FileType::Document,
        request.new_parent,
    )
    .await;
    let new_version = result.map_err(|e| match e {
        index_db::FileError::DoesNotExist => MoveDocumentError::DocumentNotFound,
        index_db::FileError::IncorrectOldVersion => MoveDocumentError::EditConflict,
        index_db::FileError::Deleted => MoveDocumentError::DocumentDeleted,
        index_db::FileError::PathTaken => MoveDocumentError::DocumentPathTaken,
        _ => {
            println!("Internal server error! {:?}", e);
            MoveDocumentError::InternalError
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(MoveDocumentResponse {
            new_metadata_version: new_version,
        }),
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

    let result = index_db::rename_file(
        &transaction,
        request.id,
        request.old_metadata_version,
        FileType::Document,
        &request.new_name,
    )
    .await;
    let new_version = result.map_err(|e| match e {
        index_db::FileError::DoesNotExist => RenameDocumentError::DocumentNotFound,
        index_db::FileError::IncorrectOldVersion => RenameDocumentError::EditConflict,
        index_db::FileError::Deleted => RenameDocumentError::DocumentDeleted,
        _ => {
            println!("Internal server error! {:?}", e);
            RenameDocumentError::InternalError
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(RenameDocumentResponse {
            new_metadata_version: new_version,
        }),
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(RenameDocumentError::InternalError)
        }
    }
}

pub async fn create_folder(
    server_state: &mut ServerState,
    request: CreateFolderRequest,
) -> Result<CreateFolderResponse, CreateFolderError> {
    if !username_is_valid(&request.username) {
        return Err(CreateFolderError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(CreateFolderError::InternalError);
        }
    };

    let result = index_db::create_folder(
        &transaction,
        request.id,
        request.parent,
        &request.name,
        &request.username,
        &request.signature,
    )
    .await;
    let new_version = result.map_err(|e| match e {
        index_db::FileError::IdTaken => CreateFolderError::FileIdTaken,
        index_db::FileError::PathTaken => CreateFolderError::FolderPathTaken,
        _ => {
            println!("Internal server error! {:?}", e);
            CreateFolderError::InternalError
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(CreateFolderResponse {
            new_metadata_version: new_version,
        }),
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(CreateFolderError::InternalError)
        }
    }
}

pub async fn delete_folder(
    server_state: &mut ServerState,
    request: DeleteFolderRequest,
) -> Result<DeleteFolderResponse, DeleteFolderError> {
    if !username_is_valid(&request.username) {
        return Err(DeleteFolderError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(DeleteFolderError::InternalError);
        }
    };

    let result =
        index_db::delete_file(&transaction, request.id, request.old_metadata_version, FileType::Folder).await;
    let (_, new_version) = result.map_err(|e| match e {
        index_db::FileError::DoesNotExist => DeleteFolderError::FolderNotFound,
        index_db::FileError::IncorrectOldVersion => DeleteFolderError::EditConflict,
        index_db::FileError::Deleted => DeleteFolderError::FolderDeleted,
        _ => {
            println!("Internal server error! {:?}", e);
            DeleteFolderError::InternalError
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(DeleteFolderResponse {
            new_metadata_version: new_version,
        }),
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(DeleteFolderError::InternalError)
        }
    }
}

pub async fn move_folder(
    server_state: &mut ServerState,
    request: MoveFolderRequest,
) -> Result<MoveFolderResponse, MoveFolderError> {
    if !username_is_valid(&request.username) {
        return Err(MoveFolderError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(MoveFolderError::InternalError);
        }
    };

    let result = index_db::move_file(
        &transaction,
        request.id,
        request.old_metadata_version,
        FileType::Folder,
        request.new_parent,
    )
    .await;
    let new_version = result.map_err(|e| match e {
        index_db::FileError::DoesNotExist => MoveFolderError::FolderNotFound,
        index_db::FileError::IncorrectOldVersion => MoveFolderError::EditConflict,
        index_db::FileError::Deleted => MoveFolderError::FolderDeleted,
        index_db::FileError::PathTaken => MoveFolderError::FolderPathTaken,
        _ => {
            println!("Internal server error! {:?}", e);
            MoveFolderError::InternalError
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(MoveFolderResponse {
            new_metadata_version: new_version,
        }),
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(MoveFolderError::InternalError)
        }
    }
}

pub async fn rename_folder(
    server_state: &mut ServerState,
    request: RenameFolderRequest,
) -> Result<RenameFolderResponse, RenameFolderError> {
    if !username_is_valid(&request.username) {
        return Err(RenameFolderError::InvalidUsername);
    }
    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(RenameFolderError::InternalError);
        }
    };

    let result = index_db::rename_file(
        &transaction,
        request.id,
        request.old_metadata_version,
        FileType::Folder,
        &request.new_name,
    )
    .await;
    let new_version = result.map_err(|e| match e {
        index_db::FileError::DoesNotExist => RenameFolderError::FolderNotFound,
        index_db::FileError::IncorrectOldVersion => RenameFolderError::EditConflict,
        index_db::FileError::Deleted => RenameFolderError::FolderDeleted,
        _ => {
            println!("Internal server error! {:?}", e);
            RenameFolderError::InternalError
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(RenameFolderResponse {
            new_metadata_version: new_version,
        }),
        Err(e) => {
            error!("Internal server error! Cannot commit transaction: {:?}", e);
            Err(RenameFolderError::InternalError)
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
    let result = index_db::get_updates(
        &transaction,
        &request.username,
        request.since_metadata_version,
    )
    .await;
    let updates = result.map_err(|e| {
        error!("Internal server error! {:?}", e);
        GetUpdatesError::InternalError
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(GetUpdatesResponse {
            file_metadata: updates,
        }),
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
    // let auth = serde_json::from_str::<SignedValue>(&request.auth)
    //     .map_err(|_| NewAccountError::InvalidAuth)?;
    // RsaImpl::verify(&request.public_key, &auth).map_err(|_| NewAccountError::InvalidPublicKey)?;
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

    let result = index_db::new_account(
        &transaction,
        &request.username,
        &serde_json::to_string(&request.public_key)
            .map_err(|_| NewAccountError::InvalidPublicKey)?,
    )
    .await;
    result.map_err(|e| match e {
        index_db::AccountError::UsernameTaken => NewAccountError::UsernameTaken,
        _ => {
            println!("Internal server error! {:?}", e);
            NewAccountError::InternalError
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(NewAccountResponse {}),
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
    let result = index_db::get_public_key(&transaction, &request.username).await;
    let key = result.map_err(|e| match e {
        index_db::PublicKeyError::UserNotFound => GetPublicKeyError::UserNotFound,
        _ => {
            println!("Internal server error! {:?}", e);
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
