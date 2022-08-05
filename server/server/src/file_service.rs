use crate::ServerError;
use crate::ServerError::ClientError;
use crate::{document_service, RequestContext};
use hmdb::transaction::Transaction;
use lockbook_shared::api::*;
use lockbook_shared::clock::get_time;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{Diff, Owner};
use lockbook_shared::server_file::IntoServerFile;
use lockbook_shared::server_tree::ServerTree;
use lockbook_shared::tree_like::{Stagable, TreeLike};
use std::collections::HashSet;
use uuid::Uuid;

pub async fn upsert_file_metadata(
    context: RequestContext<'_, UpsertRequest>,
) -> Result<(), ServerError<UpsertError>> {
    let (request, server_state) = (context.request, context.server_state);
    let owner = Owner(context.public_key);
    let docs_to_delete: Result<Vec<(Uuid, [u8; 32])>, ServerError<UpsertError>> =
        context.server_state.index_db.transaction(|tx| {
            let mut tree =
                ServerTree { owner, owned: &mut tx.owned_files, metas: &mut tx.metas }.to_lazy();

            let mut prior_deleted_docs = HashSet::new();
            for id in tree.owned_ids() {
                if tree.find(&id)?.is_document() && tree.calculate_deleted(&id)? {
                    prior_deleted_docs.insert(id);
                }
            }

            let mut tree = tree.stage_diff(&owner, request.updates)?;
            tree.validate()?;
            let mut tree = tree.promote();

            let mut new_deleted = vec![];
            for id in tree.owned_ids() {
                if tree.find(&id)?.is_document()
                    && tree.calculate_deleted(&id)?
                    && !prior_deleted_docs.contains(&id)
                {
                    let meta = tree.find(&id)?;
                    if let Some(digest) = meta.file.timestamped_value.value.document_hmac {
                        tx.sizes.delete(*meta.id());
                        new_deleted.push((*meta.id(), digest));
                    }
                }
            }
            Ok(new_deleted)
        })?;

    let docs_to_delete = docs_to_delete?;

    for (id, digest) in docs_to_delete {
        document_service::delete(server_state, &id, &digest).await?;
    }
    Ok(())
}

pub async fn change_doc(
    context: RequestContext<'_, ChangeDocRequest>,
) -> Result<(), ServerError<ChangeDocError>> {
    use ChangeDocError::*;

    let (request, server_state) = (context.request, context.server_state);
    let request_pk = Owner(context.public_key);

    // Validate Diff
    if request.diff.diff() != vec![Diff::Hmac] {
        return Err(ClientError(DiffMalformed));
    }

    if let Some(old) = &request.diff.old {
        if old.id() != request.diff.new.id() {
            return Err(ClientError(DiffMalformed));
        }
    }

    if request.diff.new.document_hmac().is_none() {
        return Err(ClientError(HmacMissing));
    }

    context.server_state.index_db.transaction(|tx| {
        let mut tree =
            ServerTree { owner: request_pk, owned: &mut tx.owned_files, metas: &mut tx.metas }
                .to_lazy();

        let meta = &tree
            .maybe_find(request.diff.new.id())
            .ok_or(ClientError(DocumentNotFound))?
            .file;

        if let Some(old) = &request.diff.old {
            if meta != old {
                return Err(ClientError(OldVersionIncorrect));
            }
        }

        // Maybe a moot check now that the tree is constructed based on ownership
        if meta.owner() != request_pk {
            return Err(ClientError(NotPermissioned));
        }

        if tree.calculate_deleted(request.diff.new.id())? {
            return Err(ClientError(DocumentDeleted));
        }

        // Here is where you would check if the person is out of space as a result of the new file.
        // You could make this a transaction and check whether or not this is an increase in size or
        // a reduction
        Ok(())
    })??;

    let new_version = get_time().0 as u64;
    let new = request.diff.new.clone().add_time(new_version);
    document_service::insert(
        server_state,
        request.diff.new.id(),
        request.diff.new.document_hmac().unwrap(),
        &request.new_content,
    )
    .await?;

    let result = server_state.index_db.transaction(|tx| {
        let mut tree =
            ServerTree { owner: request_pk, owned: &mut tx.owned_files, metas: &mut tx.metas }
                .to_lazy();
        let new_size = request.new_content.value.len() as u64;

        if tree.calculate_deleted(request.diff.new.id())? {
            return Err(ClientError(DocumentDeleted));
        }

        let meta = &tree
            .maybe_find(request.diff.new.id())
            .ok_or(ClientError(DocumentNotFound))?
            .file;

        if let Some(old) = &request.diff.old {
            if meta != old {
                return Err(ClientError(OldVersionIncorrect));
            }
        }

        if let Some(old) = &request.diff.old {
            if meta != old {
                return Err(ClientError(OldVersionIncorrect));
            }
        }

        tx.sizes.insert(*meta.id(), new_size);
        tree.stage(vec![new]).promote();

        Ok(())
    })?;

    if result.is_err() {
        // Cleanup the NEW file created if, for some reason, the tx failed
        document_service::delete(
            server_state,
            request.diff.new.id(),
            request.diff.new.document_hmac().unwrap(),
        )
        .await?;
    }

    result?;

    // New
    if let Some(hmac) = request.diff.old.unwrap().document_hmac() {
        document_service::delete(server_state, request.diff.new.id(), hmac).await?;
    }

    Ok(())
}

pub async fn get_document(
    context: RequestContext<'_, GetDocRequest>,
) -> Result<GetDocumentResponse, ServerError<GetDocumentError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let meta = server_state
        .index_db
        .metas
        .get(&request.id)?
        .ok_or(ClientError(GetDocumentError::DocumentNotFound))?;

    if meta.owner().0 != context.public_key {
        return Err(ClientError(GetDocumentError::NotPermissioned));
    }

    let hmac = meta
        .document_hmac()
        .ok_or(ClientError(GetDocumentError::DocumentNotFound))?;

    if request.hmac != *hmac {
        return Err(ClientError(GetDocumentError::DocumentNotFound));
    }

    let content = document_service::get(server_state, &request.id, &request.hmac).await?;

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
            .iter()
            .filter_map(|id| tx.metas.get(id))
            .filter(|meta| meta.version > request.since_metadata_version)
            .map(|meta| meta.file.clone())
            .collect();

        let as_of_metadata_version = get_time().0 as u64;
        Ok(GetUpdatesResponse { as_of_metadata_version, file_metadata })
    })?
}
