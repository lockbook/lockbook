use crate::file_content_client;
use crate::file_index_repo;
use crate::file_index_repo::FileError;
use crate::usage_service;
use crate::utils::{username_is_valid, version_is_supported};
use crate::ServerState;
use lockbook_core::model::api::*;
use lockbook_core::model::file_metadata::FileType;

pub async fn change_document_content(
    server_state: &mut ServerState,
    request: ChangeDocumentContentRequest,
) -> Result<ChangeDocumentContentResponse, ChangeDocumentContentError> {
    if !version_is_supported(&request.client_version) {
        return Err(ChangeDocumentContentError::ClientUpdateRequired);
    }

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

    let result = file_index_repo::change_document_content_version(
        &transaction,
        request.id,
        request.old_metadata_version,
    )
    .await;

    let (old_content_version, new_version) = result.map_err(|e| match e {
        FileError::DoesNotExist => ChangeDocumentContentError::DocumentNotFound,
        FileError::IncorrectOldVersion => ChangeDocumentContentError::EditConflict,
        FileError::Deleted => ChangeDocumentContentError::DocumentDeleted,
        FileError::FolderMovedIntoDescendants
        | FileError::Deserialize(_)
        | FileError::IdTaken
        | FileError::OwnerDoesNotExist
        | FileError::ParentDoesNotExist
        | FileError::ParentDeleted
        | FileError::IllegalRootChange
        | FileError::PathTaken
        | FileError::Postgres(_)
        | FileError::Serialize(_)
        | FileError::WrongFileType
        | FileError::Unknown(_) => {
            error!(
                "Internal server error! Cannot change document content version in Postgres: {:?}",
                e
            );
            ChangeDocumentContentError::InternalError
        }
    })?;

    let create_result = file_content_client::create(
        &server_state.files_db_client,
        request.id,
        new_version,
        &request.new_content,
    )
    .await;
    if create_result.is_err() {
        error!(
            "Internal server error! Cannot create file in S3: {:?}",
            create_result
        );
        return Err(ChangeDocumentContentError::InternalError);
    };

    let delete_result = file_content_client::delete(
        &server_state.files_db_client,
        request.id,
        old_content_version,
    )
    .await;
    if delete_result.is_err() {
        error!(
            "Internal server error! Cannot delete file in S3: {:?}",
            delete_result
        );
        return Err(ChangeDocumentContentError::InternalError);
    };

    usage_service::track_content_change(
        &transaction,
        &request.id,
        &request.username,
        &request.new_content,
    )
    .await
    .map_err(|err| {
        error!("Usage tracking error: {:?}", err);
        ChangeDocumentContentError::InternalError
    })?;

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
    if !version_is_supported(&request.client_version) {
        return Err(CreateDocumentError::ClientUpdateRequired);
    }

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

    let index_result = file_index_repo::create_file(
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
        FileError::IdTaken => CreateDocumentError::FileIdTaken,
        FileError::PathTaken => CreateDocumentError::DocumentPathTaken,
        FileError::OwnerDoesNotExist => CreateDocumentError::UserNotFound,
        FileError::ParentDoesNotExist => CreateDocumentError::ParentNotFound,
        FileError::Deleted
        | FileError::Deserialize(_)
        | FileError::DoesNotExist
        | FileError::IncorrectOldVersion
        | FileError::ParentDeleted
        | FileError::FolderMovedIntoDescendants
        | FileError::IllegalRootChange
        | FileError::Postgres(_)
        | FileError::Serialize(_)
        | FileError::WrongFileType
        | FileError::Unknown(_) => {
            error!(
                "Internal server error! Cannot create document in Postgres: {:?}",
                e
            );
            CreateDocumentError::InternalError
        }
    })?;

    usage_service::track_content_change(
        &transaction,
        &request.id,
        &request.username,
        &request.content,
    )
    .await
    .map_err(|err| {
        error!("Usage tracking error: {:?}", err);
        CreateDocumentError::InternalError
    })?;

    let files_result = file_content_client::create(
        &server_state.files_db_client,
        request.id,
        new_version,
        &request.content,
    )
    .await;

    if files_result.is_err() {
        error!(
            "Internal server error! Cannot create file in S3: {:?}",
            files_result
        );
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
    if !version_is_supported(&request.client_version) {
        return Err(DeleteDocumentError::ClientUpdateRequired);
    }

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
        file_index_repo::delete_file(&transaction, request.id, FileType::Document).await;
    let index_responses = index_result.map_err(|e| match e {
        FileError::DoesNotExist => DeleteDocumentError::DocumentNotFound,
        FileError::IncorrectOldVersion => DeleteDocumentError::EditConflict,
        FileError::Deleted => DeleteDocumentError::DocumentDeleted,
        FileError::Deserialize(_)
        | FileError::IdTaken
        | FileError::OwnerDoesNotExist
        | FileError::ParentDoesNotExist
        | FileError::ParentDeleted
        | FileError::FolderMovedIntoDescendants
        | FileError::IllegalRootChange
        | FileError::PathTaken
        | FileError::Postgres(_)
        | FileError::Serialize(_)
        | FileError::WrongFileType
        | FileError::Unknown(_) => {
            error!(
                "Internal server error! Cannot delete document in Postgres: {:?}",
                e
            );
            DeleteDocumentError::InternalError
        }
    })?;

    let single_index_response = if let Some(result) = index_responses.responses.iter().last() {
        result
    } else {
        error!("Internal server error! Unexpected zero or multiple postgres rows");
        return Err(DeleteDocumentError::InternalError);
    };

    let files_result = file_content_client::delete(
        &server_state.files_db_client,
        request.id,
        single_index_response.old_content_version,
    )
    .await;
    if files_result.is_err() {
        error!(
            "Internal server error! Cannot delete file in S3: {:?}",
            files_result
        );
        return Err(DeleteDocumentError::InternalError);
    };

    usage_service::track_deletion(&transaction, &request.id, &request.username)
        .await
        .map_err(|err| {
            error!("Usage tracking error: {:?}", err);
            DeleteDocumentError::InternalError
        })?;

    match transaction.commit().await {
        Ok(()) => Ok(DeleteDocumentResponse {
            new_metadata_and_content_version: single_index_response.new_metadata_version,
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
    if !version_is_supported(&request.client_version) {
        return Err(MoveDocumentError::ClientUpdateRequired);
    }

    if !username_is_valid(&request.username) {
        return Err(MoveDocumentError::InvalidUsername);
    }

    if request.id == request.new_parent {
        return Err(MoveDocumentError::FolderMovedIntoItself);
    }

    let transaction = match server_state.index_db_client.transaction().await {
        Ok(t) => t,
        Err(e) => {
            error!("Internal server error! Cannot begin transaction: {:?}", e);
            return Err(MoveDocumentError::InternalError);
        }
    };

    let result = file_index_repo::move_file(
        &transaction,
        request.id,
        request.old_metadata_version,
        FileType::Document,
        request.new_parent,
        request.new_folder_access,
    )
    .await;
    let new_version = result.map_err(|e| match e {
        FileError::DoesNotExist => MoveDocumentError::DocumentNotFound,
        FileError::IncorrectOldVersion => MoveDocumentError::EditConflict,
        FileError::Deleted => MoveDocumentError::DocumentDeleted,
        FileError::PathTaken => MoveDocumentError::DocumentPathTaken,
        FileError::ParentDoesNotExist => MoveDocumentError::ParentNotFound,
        FileError::ParentDeleted => MoveDocumentError::ParentDeleted,
        FileError::Deserialize(_)
        | FileError::IdTaken
        | FileError::OwnerDoesNotExist
        | FileError::FolderMovedIntoDescendants
        | FileError::IllegalRootChange
        | FileError::Postgres(_)
        | FileError::Serialize(_)
        | FileError::WrongFileType
        | FileError::Unknown(_) => {
            error!(
                "Internal server error! Cannot move document in Postgres: {:?}",
                e
            );
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
    if !version_is_supported(&request.client_version) {
        return Err(RenameDocumentError::ClientUpdateRequired);
    }

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

    let result = file_index_repo::rename_file(
        &transaction,
        request.id,
        request.old_metadata_version,
        FileType::Document,
        &request.new_name,
    )
    .await;
    let new_version = result.map_err(|e| match e {
        FileError::DoesNotExist => RenameDocumentError::DocumentNotFound,
        FileError::IncorrectOldVersion => RenameDocumentError::EditConflict,
        FileError::Deleted => RenameDocumentError::DocumentDeleted,
        FileError::PathTaken => RenameDocumentError::DocumentPathTaken,
        FileError::Deserialize(_)
        | FileError::IdTaken
        | FileError::OwnerDoesNotExist
        | FileError::ParentDoesNotExist
        | FileError::ParentDeleted
        | FileError::FolderMovedIntoDescendants
        | FileError::IllegalRootChange
        | FileError::Postgres(_)
        | FileError::Serialize(_)
        | FileError::WrongFileType
        | FileError::Unknown(_) => {
            error!(
                "Internal server error! Cannot rename document in Postgres: {:?}",
                e
            );
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

pub async fn get_document(
    server_state: &mut ServerState,
    request: GetDocumentRequest,
) -> Result<GetDocumentResponse, GetDocumentError> {
    if !version_is_supported(&request.client_version) {
        return Err(GetDocumentError::ClientUpdateRequired);
    }

    let files_result = file_content_client::get(
        &server_state.files_db_client,
        request.id,
        request.content_version,
    )
    .await;
    match files_result {
        Ok(c) => Ok(GetDocumentResponse { content: c }),
        Err(file_content_client::Error::NoSuchKey(_)) => Err(GetDocumentError::DocumentNotFound),
        Err(e) => {
            error!("Internal server error! Cannot get file from S3: {:?}", e);
            Err(GetDocumentError::InternalError)
        }
    }
}

pub async fn create_folder(
    server_state: &mut ServerState,
    request: CreateFolderRequest,
) -> Result<CreateFolderResponse, CreateFolderError> {
    if !version_is_supported(&request.client_version) {
        return Err(CreateFolderError::ClientUpdateRequired);
    }

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

    let result = file_index_repo::create_file(
        &transaction,
        request.id,
        request.parent,
        FileType::Folder,
        &request.name,
        &request.username,
        &request.signature,
        &request.parent_access_key,
    )
    .await;
    let new_version = result.map_err(|e| match e {
        FileError::IdTaken => CreateFolderError::FileIdTaken,
        FileError::PathTaken => CreateFolderError::FolderPathTaken,
        FileError::OwnerDoesNotExist => CreateFolderError::UserNotFound,
        FileError::ParentDoesNotExist => CreateFolderError::ParentNotFound,
        FileError::Deleted
        | FileError::Deserialize(_)
        | FileError::DoesNotExist
        | FileError::IncorrectOldVersion
        | FileError::ParentDeleted
        | FileError::FolderMovedIntoDescendants
        | FileError::IllegalRootChange
        | FileError::Postgres(_)
        | FileError::Serialize(_)
        | FileError::WrongFileType
        | FileError::Unknown(_) => {
            error!(
                "Internal server error! Cannot create folder in Postgres: {:?}",
                e
            );
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
    if !version_is_supported(&request.client_version) {
        return Err(DeleteFolderError::ClientUpdateRequired);
    }

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

    let index_result =
        file_index_repo::delete_file(&transaction, request.id, FileType::Folder).await;
    let index_responses = index_result.map_err(|e| match e {
        FileError::DoesNotExist => DeleteFolderError::FolderNotFound,
        FileError::IncorrectOldVersion => DeleteFolderError::EditConflict,
        FileError::Deleted => DeleteFolderError::FolderDeleted,
        FileError::Deserialize(_)
        | FileError::IdTaken
        | FileError::OwnerDoesNotExist
        | FileError::ParentDoesNotExist
        | FileError::ParentDeleted
        | FileError::FolderMovedIntoDescendants
        | FileError::IllegalRootChange
        | FileError::PathTaken
        | FileError::Postgres(_)
        | FileError::Serialize(_)
        | FileError::WrongFileType
        | FileError::Unknown(_) => {
            error!(
                "Internal server error! Cannot delete folder in Postgres: {:?}",
                e
            );
            DeleteFolderError::InternalError
        }
    })?;

    let root_result = if let Some(result) = index_responses
        .responses
        .iter()
        .filter(|r| r.id == request.id)
        .last()
    {
        result
    } else {
        error!("Internal server error! Unexpected zero or multiple postgres rows for delete folder root");
        return Err(DeleteFolderError::InternalError);
    };

    for r in index_responses.responses.iter() {
        if !r.is_folder {
            let files_result = file_content_client::delete(
                &server_state.files_db_client,
                r.id,
                r.old_content_version,
            )
            .await;
            if files_result.is_err() {
                error!(
                    "Internal server error! Cannot delete file in S3: {:?}",
                    files_result
                );
                return Err(DeleteFolderError::InternalError);
            };

            usage_service::track_deletion(&transaction, &r.id, &request.username)
                .await
                .map_err(|err| {
                    error!("Usage tracking error: {:?}", err);
                    DeleteFolderError::InternalError
                })?;
        }
    }

    match transaction.commit().await {
        Ok(()) => Ok(DeleteFolderResponse {
            new_metadata_version: root_result.new_metadata_version,
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
    if !version_is_supported(&request.client_version) {
        return Err(MoveFolderError::ClientUpdateRequired);
    }

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

    let result = file_index_repo::move_file(
        &transaction,
        request.id,
        request.old_metadata_version,
        FileType::Folder,
        request.new_parent,
        request.new_folder_access,
    )
    .await;
    let new_version = result.map_err(|e| match e {
        file_index_repo::FileError::DoesNotExist => MoveFolderError::FolderNotFound,
        file_index_repo::FileError::IncorrectOldVersion => MoveFolderError::EditConflict,
        file_index_repo::FileError::Deleted => MoveFolderError::FolderDeleted,
        file_index_repo::FileError::PathTaken => MoveFolderError::FolderPathTaken,
        file_index_repo::FileError::ParentDoesNotExist => MoveFolderError::ParentNotFound,
        file_index_repo::FileError::IllegalRootChange => MoveFolderError::CannotMoveRoot,
        file_index_repo::FileError::FolderMovedIntoDescendants => {
            MoveFolderError::CannotMoveIntoDescendant
        }
        file_index_repo::FileError::ParentDeleted => MoveFolderError::ParentDeleted,
        FileError::Deserialize(_)
        | FileError::IdTaken
        | FileError::OwnerDoesNotExist
        | FileError::Postgres(_)
        | FileError::Serialize(_)
        | FileError::WrongFileType
        | FileError::Unknown(_) => {
            error!(
                "Internal server error! Cannot move folder in Postgres: {:?}",
                e
            );
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
    if !version_is_supported(&request.client_version) {
        return Err(RenameFolderError::ClientUpdateRequired);
    }

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

    let result = file_index_repo::rename_file(
        &transaction,
        request.id,
        request.old_metadata_version,
        FileType::Folder,
        &request.new_name,
    )
    .await;
    let new_version = result.map_err(|e| match e {
        FileError::DoesNotExist => RenameFolderError::FolderNotFound,
        FileError::IncorrectOldVersion => RenameFolderError::EditConflict,
        FileError::IllegalRootChange => RenameFolderError::CannotRenameRoot,
        FileError::Deleted => RenameFolderError::FolderDeleted,
        FileError::PathTaken => RenameFolderError::FolderPathTaken,
        FileError::Deserialize(_)
        | FileError::IdTaken
        | FileError::OwnerDoesNotExist
        | FileError::ParentDoesNotExist
        | FileError::ParentDeleted
        | FileError::FolderMovedIntoDescendants
        | FileError::Postgres(_)
        | FileError::Serialize(_)
        | FileError::WrongFileType
        | FileError::Unknown(_) => {
            error!(
                "Internal server error! Cannot rename folder in Postgres: {:?}",
                e
            );
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
    if !version_is_supported(&request.client_version) {
        return Err(GetUpdatesError::ClientUpdateRequired);
    }

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
    let result = file_index_repo::get_updates(
        &transaction,
        &request.username,
        request.since_metadata_version,
    )
    .await;
    let updates = result.map_err(|e| {
        error!(
            "Internal server error! Cannot get updates from Postgres: {:?}",
            e
        );
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
