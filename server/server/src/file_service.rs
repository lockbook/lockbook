use crate::file_index_repo;
use crate::file_index_repo::{
    ChangeDocumentVersionAndSizeError, CreateFileError, DeleteFileError, MoveFileError,
    RenameFileError,
};
use crate::{file_content_client, RequestContext};
use lockbook_models::api::*;
use lockbook_models::file_metadata::FileType;

pub async fn change_document_content(
    context: &mut RequestContext<'_, ChangeDocumentContentRequest>,
) -> Result<ChangeDocumentContentResponse, Result<ChangeDocumentContentError, String>> {
    let request = &context.request;
    let server_state = &mut context.server_state;
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let result = file_index_repo::change_document_version_and_size(
        &mut transaction,
        request.id,
        request.new_content.value.len() as u64,
        request.old_metadata_version,
    )
    .await;

    let (old_content_version, new_version) = result.map_err(|e| match e {
        ChangeDocumentVersionAndSizeError::DoesNotExist => {
            Ok(ChangeDocumentContentError::DocumentNotFound)
        }
        ChangeDocumentVersionAndSizeError::IncorrectOldVersion => {
            Ok(ChangeDocumentContentError::EditConflict)
        }
        ChangeDocumentVersionAndSizeError::Deleted => {
            Ok(ChangeDocumentContentError::DocumentDeleted)
        }
        ChangeDocumentVersionAndSizeError::Postgres(_)
        | ChangeDocumentVersionAndSizeError::Deserialize(_) => Err(format!(
            "Cannot change document content version in Postgres: {:?}",
            e
        )),
    })?;

    let create_result = file_content_client::create(
        &server_state.files_db_client,
        request.id,
        new_version,
        &request.new_content,
    )
    .await;
    if create_result.is_err() {
        return Err(Err(format!(
            "Cannot create file in S3: {:?}",
            create_result
        )));
    };

    let delete_result = file_content_client::delete(
        &server_state.files_db_client,
        request.id,
        old_content_version,
    )
    .await;
    if delete_result.is_err() {
        return Err(Err(format!(
            "Cannot delete file in S3: {:?}",
            delete_result
        )));
    };

    match transaction.commit().await {
        Ok(()) => Ok(ChangeDocumentContentResponse {
            new_metadata_and_content_version: new_version,
        }),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn create_document(
    context: &mut RequestContext<'_, CreateDocumentRequest>,
) -> Result<CreateDocumentResponse, Result<CreateDocumentError, String>> {
    let request = &context.request;
    let server_state = &mut context.server_state;
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let index_result = file_index_repo::create_file(
        &mut transaction,
        request.id,
        request.parent,
        FileType::Document,
        &request.name,
        &context.public_key,
        &request.parent_access_key,
        Some(request.content.value.len() as u64),
    )
    .await;
    let new_version = index_result.map_err(|e| match e {
        CreateFileError::IdTaken => Ok(CreateDocumentError::FileIdTaken),
        CreateFileError::PathTaken => Ok(CreateDocumentError::DocumentPathTaken),
        CreateFileError::OwnerDoesNotExist => Ok(CreateDocumentError::UserNotFound),
        CreateFileError::ParentDoesNotExist => Ok(CreateDocumentError::ParentNotFound),
        CreateFileError::AncestorDeleted => Ok(CreateDocumentError::AncestorDeleted),
        CreateFileError::Postgres(_) | CreateFileError::Serialize(_) => {
            Err(format!("Cannot create document in Postgres: {:?}", e))
        }
    })?;

    let files_result = file_content_client::create(
        &server_state.files_db_client,
        request.id,
        new_version,
        &request.content,
    )
    .await;

    if files_result.is_err() {
        return Err(Err(format!("Cannot create file in S3: {:?}", files_result)));
    };

    match transaction.commit().await {
        Ok(()) => Ok(CreateDocumentResponse {
            new_metadata_and_content_version: new_version,
        }),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn delete_document(
    context: &mut RequestContext<'_, DeleteDocumentRequest>,
) -> Result<DeleteDocumentResponse, Result<DeleteDocumentError, String>> {
    let request = &context.request;
    let server_state = &mut context.server_state;
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let index_result = file_index_repo::delete_file(&mut transaction, request.id).await;
    let index_responses = index_result.map_err(|e| match e {
        DeleteFileError::DoesNotExist => Ok(DeleteDocumentError::DocumentNotFound),
        DeleteFileError::Deleted => Ok(DeleteDocumentError::DocumentDeleted),
        DeleteFileError::IllegalRootChange
        | DeleteFileError::Postgres(_)
        | DeleteFileError::Serialize(_)
        | DeleteFileError::Deserialize(_)
        | DeleteFileError::UuidDeserialize(_) => {
            Err(format!("Cannot delete document in Postgres: {:?}", e))
        }
    })?;

    let single_index_response = if let Some(result) = index_responses.iter().last() {
        result
    } else {
        return Err(Err(String::from(
            "Unexpected zero or multiple postgres rows during delete document",
        )));
    };

    let files_result = file_content_client::delete(
        &server_state.files_db_client,
        request.id,
        single_index_response.old_content_version,
    )
    .await;
    if files_result.is_err() {
        return Err(Err(format!("Cannot delete file in S3: {:?}", files_result)));
    };

    match transaction.commit().await {
        Ok(()) => Ok(DeleteDocumentResponse {
            new_metadata_and_content_version: single_index_response.new_metadata_version,
        }),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn move_document(
    context: &mut RequestContext<'_, MoveDocumentRequest>,
) -> Result<MoveDocumentResponse, Result<MoveDocumentError, String>> {
    let request = &context.request;
    let server_state = &mut context.server_state;
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let result = file_index_repo::move_file(
        &mut transaction,
        request.id,
        request.old_metadata_version,
        request.new_parent,
        request.new_folder_access.clone(),
    )
    .await;
    let new_version = result.map_err(|e| match e {
        MoveFileError::DoesNotExist => Ok(MoveDocumentError::DocumentNotFound),
        MoveFileError::IncorrectOldVersion => Ok(MoveDocumentError::EditConflict),
        MoveFileError::Deleted => Ok(MoveDocumentError::DocumentDeleted),
        MoveFileError::PathTaken => Ok(MoveDocumentError::DocumentPathTaken),
        MoveFileError::ParentDoesNotExist => Ok(MoveDocumentError::ParentNotFound),
        MoveFileError::ParentDeleted => Ok(MoveDocumentError::ParentDeleted),
        MoveFileError::FolderMovedIntoDescendants
        | MoveFileError::IllegalRootChange
        | MoveFileError::Postgres(_)
        | MoveFileError::Serialize(_) => Err(format!("Cannot move document in Postgres: {:?}", e)),
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(MoveDocumentResponse {
            new_metadata_version: new_version,
        }),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn rename_document(
    context: &mut RequestContext<'_, RenameDocumentRequest>,
) -> Result<RenameDocumentResponse, Result<RenameDocumentError, String>> {
    let request = &context.request;
    let server_state = &mut context.server_state;
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let result = file_index_repo::rename_file(
        &mut transaction,
        request.id,
        request.old_metadata_version,
        FileType::Document,
        &request.new_name,
    )
    .await;
    let new_version = result.map_err(|e| match e {
        RenameFileError::DoesNotExist => Ok(RenameDocumentError::DocumentNotFound),
        RenameFileError::IncorrectOldVersion => Ok(RenameDocumentError::EditConflict),
        RenameFileError::Deleted => Ok(RenameDocumentError::DocumentDeleted),
        RenameFileError::PathTaken => Ok(RenameDocumentError::DocumentPathTaken),
        RenameFileError::IllegalRootChange
        | RenameFileError::Postgres(_)
        | RenameFileError::Serialize(_) => {
            Err(format!("Cannot rename document in Postgres: {:?}", e))
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(RenameDocumentResponse {
            new_metadata_version: new_version,
        }),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn get_document(
    context: &mut RequestContext<'_, GetDocumentRequest>,
) -> Result<GetDocumentResponse, Result<GetDocumentError, String>> {
    let request = &context.request;
    let server_state = &mut context.server_state;
    let files_result = file_content_client::get(
        &server_state.files_db_client,
        request.id,
        request.content_version,
    )
    .await;
    match files_result {
        Ok(c) => Ok(GetDocumentResponse { content: c }),
        Err(file_content_client::Error::NoSuchKey(_)) => {
            Err(Ok(GetDocumentError::DocumentNotFound))
        }
        Err(e) => Err(Err(format!("Cannot get file from S3: {:?}", e))),
    }
}

pub async fn create_folder(
    context: &mut RequestContext<'_, CreateFolderRequest>,
) -> Result<CreateFolderResponse, Result<CreateFolderError, String>> {
    let request = &context.request;
    let server_state = &mut context.server_state;
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let result = file_index_repo::create_file(
        &mut transaction,
        request.id,
        request.parent,
        FileType::Folder,
        &request.name,
        &context.public_key,
        &request.parent_access_key,
        None,
    )
    .await;
    let new_version = result.map_err(|e| match e {
        CreateFileError::IdTaken => Ok(CreateFolderError::FileIdTaken),
        CreateFileError::PathTaken => Ok(CreateFolderError::FolderPathTaken),
        CreateFileError::OwnerDoesNotExist => Ok(CreateFolderError::UserNotFound),
        CreateFileError::ParentDoesNotExist => Ok(CreateFolderError::ParentNotFound),
        CreateFileError::AncestorDeleted => Ok(CreateFolderError::AncestorDeleted),
        CreateFileError::Postgres(_) | CreateFileError::Serialize(_) => {
            Err(format!("Cannot create folder in Postgres: {:?}", e))
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(CreateFolderResponse {
            new_metadata_version: new_version,
        }),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn delete_folder(
    context: &mut RequestContext<'_, DeleteFolderRequest>,
) -> Result<DeleteFolderResponse, Result<DeleteFolderError, String>> {
    let request = &context.request;
    let server_state = &mut context.server_state;
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let index_result = file_index_repo::delete_file(&mut transaction, request.id).await;
    let index_responses = index_result.map_err(|e| match e {
        DeleteFileError::DoesNotExist => Ok(DeleteFolderError::FolderNotFound),
        DeleteFileError::Deleted => Ok(DeleteFolderError::FolderDeleted),
        DeleteFileError::IllegalRootChange => Ok(DeleteFolderError::CannotDeleteRoot),
        DeleteFileError::Postgres(_)
        | DeleteFileError::Serialize(_)
        | DeleteFileError::Deserialize(_)
        | DeleteFileError::UuidDeserialize(_) => {
            Err(format!("Cannot delete folder in Postgres: {:?}", e))
        }
    })?;

    let root_result =
        if let Some(result) = index_responses.iter().filter(|r| r.id == request.id).last() {
            result
        } else {
            return Err(Err(String::from(
                "Unexpected zero or multiple postgres rows during delete folder",
            )));
        };

    for r in index_responses.iter() {
        if !r.is_folder {
            let files_result = file_content_client::delete(
                &server_state.files_db_client,
                r.id,
                r.old_content_version,
            )
            .await;
            if files_result.is_err() {
                return Err(Err(format!("Cannot delete file in S3: {:?}", files_result)));
            };
        }
    }

    match transaction.commit().await {
        Ok(()) => Ok(DeleteFolderResponse {
            new_metadata_version: root_result.new_metadata_version,
        }),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn move_folder(
    context: &mut RequestContext<'_, MoveFolderRequest>,
) -> Result<MoveFolderResponse, Result<MoveFolderError, String>> {
    let request = &context.request;
    let server_state = &mut context.server_state;
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let result = file_index_repo::move_file(
        &mut transaction,
        request.id,
        request.old_metadata_version,
        request.new_parent,
        request.new_folder_access.clone(),
    )
    .await;
    let new_version = result.map_err(|e| match e {
        MoveFileError::DoesNotExist => Ok(MoveFolderError::FolderNotFound),
        MoveFileError::IncorrectOldVersion => Ok(MoveFolderError::EditConflict),
        MoveFileError::Deleted => Ok(MoveFolderError::FolderDeleted),
        MoveFileError::PathTaken => Ok(MoveFolderError::FolderPathTaken),
        MoveFileError::ParentDoesNotExist => Ok(MoveFolderError::ParentNotFound),
        MoveFileError::ParentDeleted => Ok(MoveFolderError::ParentDeleted),
        MoveFileError::FolderMovedIntoDescendants => Ok(MoveFolderError::CannotMoveIntoDescendant),
        MoveFileError::IllegalRootChange => Ok(MoveFolderError::CannotMoveRoot),
        MoveFileError::Postgres(_) | MoveFileError::Serialize(_) => {
            Err(format!("Cannot move folder in Postgres: {:?}", e))
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(MoveFolderResponse {
            new_metadata_version: new_version,
        }),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn rename_folder(
    context: &mut RequestContext<'_, RenameFolderRequest>,
) -> Result<RenameFolderResponse, Result<RenameFolderError, String>> {
    let request = &context.request;
    let server_state = &mut context.server_state;
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let result = file_index_repo::rename_file(
        &mut transaction,
        request.id,
        request.old_metadata_version,
        FileType::Folder,
        &request.new_name,
    )
    .await;
    let new_version = result.map_err(|e| match e {
        RenameFileError::DoesNotExist => Ok(RenameFolderError::FolderNotFound),
        RenameFileError::IncorrectOldVersion => Ok(RenameFolderError::EditConflict),
        RenameFileError::Deleted => Ok(RenameFolderError::FolderDeleted),
        RenameFileError::PathTaken => Ok(RenameFolderError::FolderPathTaken),
        RenameFileError::IllegalRootChange => Ok(RenameFolderError::CannotRenameRoot),
        RenameFileError::Postgres(_) | RenameFileError::Serialize(_) => {
            Err(format!("Cannot rename folder in Postgres: {:?}", e))
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(RenameFolderResponse {
            new_metadata_version: new_version,
        }),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn get_updates(
    context: &mut RequestContext<'_, GetUpdatesRequest>,
) -> Result<GetUpdatesResponse, Result<GetUpdatesError, String>> {
    let request = &context.request;
    let server_state = &mut context.server_state;
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };
    let result = file_index_repo::get_updates(
        &mut transaction,
        &context.public_key,
        request.since_metadata_version,
    )
    .await;
    let updates = result.map_err(|e| Err(format!("Cannot get updates from Postgres: {:?}", e)))?;

    match transaction.commit().await {
        Ok(()) => Ok(GetUpdatesResponse {
            file_metadata: updates,
        }),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}
