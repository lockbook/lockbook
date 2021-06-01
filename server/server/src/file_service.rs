use crate::file_index_repo;
use crate::file_index_repo::{
    ChangeDocumentVersionAndSizeError, CreateFileError, DeleteFileError, MoveFileError,
    RenameFileError,
};
use crate::{file_content_client, RequestContext};
use lockbook_models::api::*;
use lockbook_models::file_metadata::FileType;

pub async fn apply_changes(
    context: RequestContext<'_, Vec<FileUpdatesRequest>>,
) -> Result<(), Result<FileError, String>> {
    let (server_state, public_key) = (&context.server_state, context.public_key);
    for request in context.request {
        match request {
            FileUpdatesRequest::ChangeDocumentContentRequest(request) => {
                change_document_content(RequestContext {
                    server_state,
                    request,
                    public_key,
                })
                .await?;
            }
            FileUpdatesRequest::CreateDocumentRequest(request) => {
                create_document(RequestContext {
                    server_state,
                    request,
                    public_key,
                })
                .await?;
            }
            FileUpdatesRequest::DeleteDocumentRequest(request) => {
                delete_document(RequestContext {
                    server_state,
                    request,
                    public_key,
                })
                .await?;
            }
            FileUpdatesRequest::MoveDocumentRequest(request) => {
                move_document(RequestContext {
                    server_state,
                    request,
                    public_key,
                })
                .await?;
            }
            FileUpdatesRequest::RenameDocumentRequest(request) => {
                rename_document(RequestContext {
                    server_state,
                    request,
                    public_key,
                })
                .await?;
            }
            FileUpdatesRequest::CreateFolderRequest(request) => {
                create_folder(RequestContext {
                    server_state,
                    request,
                    public_key,
                })
                .await?;
            }
            FileUpdatesRequest::DeleteFolderRequest(request) => {
                delete_folder(RequestContext {
                    server_state,
                    request,
                    public_key,
                })
                .await?;
            }
            FileUpdatesRequest::MoveFolderRequest(request) => {
                move_folder(RequestContext {
                    server_state,
                    request,
                    public_key,
                })
                .await?;
            }
            FileUpdatesRequest::RenameFolderRequest(request) => {
                rename_folder(RequestContext {
                    server_state,
                    request,
                    public_key,
                })
                .await?;
            }
        };
    }
    Ok(())
}

pub async fn change_document_content(
    context: RequestContext<'_, ChangeDocumentContentRequest>,
) -> Result<(), Result<FileError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
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
        ChangeDocumentVersionAndSizeError::DoesNotExist => Ok(FileError::GetUpdatesRequired),
        ChangeDocumentVersionAndSizeError::IncorrectOldVersion => Ok(FileError::GetUpdatesRequired),
        ChangeDocumentVersionAndSizeError::Deleted => Ok(FileError::GetUpdatesRequired),
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
        Ok(()) => Ok(()),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn create_document(
    context: RequestContext<'_, CreateDocumentRequest>,
) -> Result<(), Result<FileError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
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
        CreateFileError::IdTaken => Ok(FileError::GetUpdatesRequired),
        CreateFileError::PathTaken => Ok(FileError::GetUpdatesRequired),
        CreateFileError::OwnerDoesNotExist => Ok(FileError::GetUpdatesRequired),
        CreateFileError::ParentDoesNotExist => Ok(FileError::GetUpdatesRequired),
        CreateFileError::AncestorDeleted => Ok(FileError::GetUpdatesRequired),
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
        Ok(()) => Ok(()),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn delete_document(
    context: RequestContext<'_, DeleteDocumentRequest>,
) -> Result<(), Result<FileError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let index_result = file_index_repo::delete_file(&mut transaction, request.id).await;
    let index_responses = index_result.map_err(|e| match e {
        DeleteFileError::DoesNotExist => Ok(FileError::GetUpdatesRequired),
        DeleteFileError::Deleted => Ok(FileError::GetUpdatesRequired),
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
        Ok(()) => Ok(()),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn move_document(
    context: RequestContext<'_, MoveDocumentRequest>,
) -> Result<(), Result<FileError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
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
    result.map_err(|e| match e {
        MoveFileError::DoesNotExist => Ok(FileError::GetUpdatesRequired),
        MoveFileError::IncorrectOldVersion => Ok(FileError::GetUpdatesRequired),
        MoveFileError::Deleted => Ok(FileError::GetUpdatesRequired),
        MoveFileError::PathTaken => Ok(FileError::GetUpdatesRequired),
        MoveFileError::ParentDoesNotExist => Ok(FileError::GetUpdatesRequired),
        MoveFileError::ParentDeleted => Ok(FileError::GetUpdatesRequired),
        MoveFileError::FolderMovedIntoDescendants
        | MoveFileError::IllegalRootChange
        | MoveFileError::Postgres(_)
        | MoveFileError::Serialize(_) => Err(format!("Cannot move document in Postgres: {:?}", e)),
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(()),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn rename_document(
    context: RequestContext<'_, RenameDocumentRequest>,
) -> Result<(), Result<FileError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
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
    result.map_err(|e| match e {
        RenameFileError::DoesNotExist => Ok(FileError::GetUpdatesRequired),
        RenameFileError::IncorrectOldVersion => Ok(FileError::GetUpdatesRequired),
        RenameFileError::Deleted => Ok(FileError::GetUpdatesRequired),
        RenameFileError::PathTaken => Ok(FileError::GetUpdatesRequired),
        RenameFileError::IllegalRootChange
        | RenameFileError::Postgres(_)
        | RenameFileError::Serialize(_) => {
            Err(format!("Cannot rename document in Postgres: {:?}", e))
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(()),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn get_document(
    context: RequestContext<'_, GetDocumentRequest>,
) -> Result<GetDocumentResponse, Result<GetDocumentError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
    let files_result = file_content_client::get(
        &server_state.files_db_client,
        request.id,
        request.content_version,
    )
    .await;
    match files_result {
        Ok(c) => Ok(GetDocumentResponse { content: c }),
        Err(file_content_client::Error::NoSuchKey(_)) => Err(Ok(GetDocumentError::DocumentNotFound)),
        Err(e) => Err(Err(format!("Cannot get file from S3: {:?}", e))),
    }
}

pub async fn create_folder(
    context: RequestContext<'_, CreateFolderRequest>,
) -> Result<(), Result<FileError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
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
    result.map_err(|e| match e {
        CreateFileError::IdTaken => Ok(FileError::GetUpdatesRequired),
        CreateFileError::PathTaken => Ok(FileError::GetUpdatesRequired),
        CreateFileError::OwnerDoesNotExist => Ok(FileError::GetUpdatesRequired),
        CreateFileError::ParentDoesNotExist => Ok(FileError::GetUpdatesRequired),
        CreateFileError::AncestorDeleted => Ok(FileError::GetUpdatesRequired),
        CreateFileError::Postgres(_) | CreateFileError::Serialize(_) => {
            Err(format!("Cannot create folder in Postgres: {:?}", e))
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(()),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn delete_folder(
    context: RequestContext<'_, DeleteFolderRequest>,
) -> Result<(), Result<FileError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut transaction = match server_state.index_db_client.begin().await {
        Ok(t) => t,
        Err(e) => {
            return Err(Err(format!("Cannot begin transaction: {:?}", e)));
        }
    };

    let index_result = file_index_repo::delete_file(&mut transaction, request.id).await;
    let index_responses = index_result.map_err(|e| match e {
        DeleteFileError::DoesNotExist => Ok(FileError::GetUpdatesRequired),
        DeleteFileError::Deleted => Ok(FileError::GetUpdatesRequired),
        DeleteFileError::IllegalRootChange => Ok(FileError::GetUpdatesRequired),
        DeleteFileError::Postgres(_)
        | DeleteFileError::Serialize(_)
        | DeleteFileError::Deserialize(_)
        | DeleteFileError::UuidDeserialize(_) => {
            Err(format!("Cannot delete folder in Postgres: {:?}", e))
        }
    })?;

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
        Ok(()) => Ok(()),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn move_folder(
    context: RequestContext<'_, MoveFolderRequest>,
) -> Result<(), Result<FileError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
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
    result.map_err(|e| match e {
        MoveFileError::DoesNotExist => Ok(FileError::GetUpdatesRequired),
        MoveFileError::IncorrectOldVersion => Ok(FileError::GetUpdatesRequired),
        MoveFileError::Deleted => Ok(FileError::GetUpdatesRequired),
        MoveFileError::PathTaken => Ok(FileError::GetUpdatesRequired),
        MoveFileError::ParentDoesNotExist => Ok(FileError::GetUpdatesRequired),
        MoveFileError::ParentDeleted => Ok(FileError::GetUpdatesRequired),
        MoveFileError::FolderMovedIntoDescendants => Ok(FileError::GetUpdatesRequired),
        MoveFileError::IllegalRootChange => Ok(FileError::GetUpdatesRequired),
        MoveFileError::Postgres(_) | MoveFileError::Serialize(_) => {
            Err(format!("Cannot move folder in Postgres: {:?}", e))
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(()),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn rename_folder(
    context: RequestContext<'_, RenameFolderRequest>,
) -> Result<(), Result<FileError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
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
    result.map_err(|e| match e {
        RenameFileError::DoesNotExist => Ok(FileError::GetUpdatesRequired),
        RenameFileError::IncorrectOldVersion => Ok(FileError::GetUpdatesRequired),
        RenameFileError::Deleted => Ok(FileError::GetUpdatesRequired),
        RenameFileError::PathTaken => Ok(FileError::GetUpdatesRequired),
        RenameFileError::IllegalRootChange => Ok(FileError::GetUpdatesRequired),
        RenameFileError::Postgres(_) | RenameFileError::Serialize(_) => {
            Err(format!("Cannot rename folder in Postgres: {:?}", e))
        }
    })?;

    match transaction.commit().await {
        Ok(()) => Ok(()),
        Err(e) => Err(Err(format!("Cannot commit transaction: {:?}", e))),
    }
}

pub async fn get_updates(
    context: RequestContext<'_, GetUpdatesRequest>,
) -> Result<GetUpdatesResponse, Result<GetUpdatesError, String>> {
    let (request, server_state) = (&context.request, context.server_state);
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
