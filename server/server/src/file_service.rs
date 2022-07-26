use crate::ServerError;
use crate::ServerError::ClientError;
use crate::Tx;
use crate::{document_service, RequestContext};
use hmdb::transaction::Transaction;
use lockbook_shared::api::FileMetadataUpsertsError::{
    GetUpdatesRequired, NewFileHasOldParentAndName, NotPermissioned, RootImmutable,
};
use lockbook_shared::api::*;
use lockbook_shared::clock::get_time;
use lockbook_shared::file_metadata::Owner;

pub async fn upsert_file_metadata(
    context: RequestContext<'_, FileMetadataUpsertsRequest>,
) -> Result<(), ServerError<FileMetadataUpsertsError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let owner = Owner(context.public_key);
    check_for_changed_root(&request.updates)?;
    let now = get_time().0 as u64;
    let docs_to_delete: Result<Vec<FileMetadata>, ServerError<FileMetadataUpsertsError>> =
        context.server_state.index_db.transaction(|tx| {
            let mut files: EncryptedFiles = tx
                .owned_files
                .get(&Owner(context.public_key))
                .ok_or(ClientError(FileMetadataUpsertsError::UserNotFound))?
                .iter()
                .filter_map(|id| tx.metas.get(id))
                .map(|f| (f.id, f))
                .collect();

            let deleted_docs = apply_changes(tx, now, &owner, &request.updates, &mut files)?;

            files
                .verify_integrity()
                .map_err(|_| ClientError(GetUpdatesRequired))?; // Could provide reject reason here

            let owned_files = files.ids();

            // TODO possibly more efficient to keep track of which id's actually changed
            for (id, file) in files {
                if file.is_deleted && file.is_document() {
                    tx.sizes.delete(id);
                }
                tx.metas.insert(id, file);
            }

            tx.owned_files.insert(owner, owned_files);

            Ok(deleted_docs)
        })?;

    let docs_to_delete = docs_to_delete?;

    for file in docs_to_delete {
        document_service::delete(server_state, file.id, file.content_version).await?;
    }
    Ok(())
}

fn check_for_changed_root(
    changes: &[FileDiff],
) -> Result<(), ServerError<FileMetadataUpsertsError>> {
    for change in changes {
        if let Some((old_parent, _)) = change.old_parent_and_name {
            if change.id == old_parent {
                return Err(ClientError(RootImmutable));
            }
            if change.id == change.new_parent {
                // TODO could be createdRoot
                return Err(ClientError(GetUpdatesRequired));
            }
        }
    }
    Ok(())
}

fn apply_changes(
    tx: &mut Tx<'_>, now: u64, owner: &Owner, changes: &[FileMetadataDiff],
    metas: &mut EncryptedFiles,
) -> Result<Vec<FileMetadata>, ServerError<FileMetadataUpsertsError>> {
    let mut deleted_documents = vec![];
    let mut new_files = vec![];
    for change in changes {
        match metas.maybe_find_mut(change.id) {
            Some(meta) => {
                meta.is_deleted = change.new_deleted;

                if let Some((old_parent, old_name)) = &change.old_parent_and_name {
                    if meta.parent != *old_parent || meta.name != *old_name {
                        return Err(ClientError(GetUpdatesRequired));
                    }
                } else {
                    // You authored a file, and you pushed it to the server, and failed to record the change
                    // And now you think this is still a new file, so you get updates
                    return Err(ClientError(GetUpdatesRequired));
                }
                meta.parent = change.new_parent;
                meta.name = change.new_name.clone();
                meta.folder_access_keys = change.new_folder_access_keys.clone();
                meta.metadata_version = now;

                if change.new_deleted && meta.is_document() {
                    deleted_documents.push(meta.clone());
                }
            }
            None => {
                if change.old_parent_and_name.is_some() {
                    return Err(ClientError(NewFileHasOldParentAndName));
                }
                let new_meta = new_meta(now, change, owner);
                if tx.metas.exists(&new_meta.id) {
                    return Err(ClientError(NotPermissioned));
                }
                new_files.push(new_meta.id);
                metas.push(new_meta);
            }
        }
    }

    let deleted_ids = metas
        .deleted_status()
        .map_err(|_| ClientError(GetUpdatesRequired))? // TODO this could be more descriptive
        .deleted;

    for id in deleted_ids {
        if let Some(deleted_fm) = metas.maybe_find_mut(id) {
            // Check if implicitly deleted
            if !deleted_fm.is_deleted {
                deleted_fm.is_deleted = true;
                deleted_fm.metadata_version = now;
                if deleted_fm.is_document() {
                    deleted_documents.push(deleted_fm.clone());
                }
            }
        }
    }

    Ok(deleted_documents)
}

fn new_meta(now: u64, diff: &FileMetadataDiff, owner: &Owner) -> FileMetadata {
    FileMetadata {
        id: diff.id,
        file_type: diff.file_type,
        parent: diff.new_parent,
        name: diff.new_name.clone(),
        owner: owner.clone(),
        metadata_version: now,
        content_version: 0,
        is_deleted: diff.new_deleted,
        user_access_keys: Default::default(),
        folder_access_keys: diff.new_folder_access_keys.clone(),
    }
}

pub async fn change_document_content(
    context: RequestContext<'_, ChangeDocumentContentRequest>,
) -> Result<ChangeDocumentContentResponse, ServerError<ChangeDocumentContentError>> {
    let (request, server_state) = (&context.request, context.server_state);
    // Ownership check
    {
        let meta = server_state
            .index_db
            .metas
            .get(&request.id)?
            .ok_or(ClientError(ChangeDocumentContentError::DocumentNotFound))?;

        if meta.owner.0 != context.public_key {
            return Err(ClientError(ChangeDocumentContentError::NotPermissioned));
        }

        // Perhaps these next two are redundant, but practically lets us boot out of this request
        // before interacting with s3
        if meta.is_deleted {
            return Err(ClientError(ChangeDocumentContentError::DocumentDeleted));
        }

        if request.old_metadata_version != meta.version {
            return Err(ClientError(ChangeDocumentContentError::EditConflict));
        }

        // Here is where you would check if the person is out of space as a result of the new file.
        // You could make this a transaction and check whether or not this is an increase in size or
        // a reduction
    }

    let new_version = get_time().0 as u64;
    let mut old_content_version = 0;
    document_service::insert(server_state, request.id, new_version, &request.new_content).await?;

    let result = server_state.index_db.transaction(|tx| {
        let new_size = request.new_content.value.len() as u64;
        let mut meta = tx
            .metas
            .get(&request.id)
            .ok_or(ClientError(ChangeDocumentContentError::DocumentNotFound))?;

        if meta.is_deleted {
            return Err(ClientError(ChangeDocumentContentError::DocumentDeleted));
        }

        if request.old_metadata_version != meta.version {
            return Err(ClientError(ChangeDocumentContentError::EditConflict));
        }

        old_content_version = meta.content_version;

        meta.content_version = new_version;
        meta.version = new_version;

        tx.sizes.insert(meta.id, new_size);
        tx.metas.insert(meta.id, meta);

        Ok(ChangeDocumentContentResponse { new_content_version: new_version })
    })?;

    if result.is_err() {
        // Cleanup the NEW file created if, for some reason, the tx failed
        document_service::delete(server_state, request.id, new_version).await?;
    }

    let result = result?;

    document_service::delete(server_state, request.id, old_content_version).await?;

    Ok(result)
}

pub async fn get_document(
    context: RequestContext<'_, GetDocumentRequest>,
) -> Result<GetDocumentResponse, ServerError<GetDocumentError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let meta = server_state
        .index_db
        .metas
        .get(&request.id)?
        .ok_or(ClientError(GetDocumentError::DocumentNotFound))?;

    if meta.owner.0 != context.public_key {
        return Err(ClientError(GetDocumentError::NotPermissioned));
    }

    let content = document_service::get(server_state, request.id, request.content_version).await?;

    Ok(GetDocumentResponse { content })
}

pub async fn get_updates(
    context: RequestContext<'_, GetUpdatesRequest>,
) -> Result<GetUpdatesResponse, ServerError<GetUpdatesError>> {
    let (request, server_state) = (&context.request, context.server_state);
    server_state.index_db.transaction(|tx| {
        let file_metadata = tx
            .owned_files
            .get(&Owner(context.public_key))
            .ok_or(ClientError(GetUpdatesError::UserNotFound))?
            .into_iter()
            .filter_map(|id| tx.metas.get(&id))
            .filter(|meta| meta.version > request.since_metadata_version)
            .collect();
        Ok(GetUpdatesResponse { file_metadata })
    })?
}
