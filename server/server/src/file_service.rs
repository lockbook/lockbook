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

pub async fn upsert_file_metadata(
    context: RequestContext<'_, UpsertRequest>,
) -> Result<(), ServerError<UpsertError>> {
    let (request, server_state) = (context.request, context.server_state);
    let req_owner = Owner(context.public_key);

    let mut prior_deleted_docs = HashSet::new();
    let mut new_deleted = vec![];

    let res: Result<(), ServerError<UpsertError>> =
        context.server_state.index_db.transaction(|tx| {
            // validate all trees
            let mut tree =
                ServerTree::new(req_owner, &mut tx.owned_files, &mut tx.metas)?.to_lazy();

            for id in tree.owned_ids() {
                if tree.find(&id)?.is_document() && tree.calculate_deleted(&id)? {
                    prior_deleted_docs.insert(id);
                }
            }

            let mut tree = tree.stage_diff(request.updates.clone())?;
            tree.validate(req_owner)?;

            let mut tree = ServerTree::new(req_owner, &mut tx.owned_files, &mut tx.metas)?
                .to_lazy()
                .stage_diff(request.updates)?
                .promote();

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
            Ok(())
        })?;

    res?;

    for (id, digest) in new_deleted {
        document_service::delete(server_state, &id, &digest).await?;
    }
    Ok(())
}

pub async fn change_doc(
    context: RequestContext<'_, ChangeDocRequest>,
) -> Result<(), ServerError<ChangeDocError>> {
    use ChangeDocError::*;

    let (request, server_state) = (context.request, context.server_state);

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

    let req_pk = context.public_key;

    context.server_state.index_db.transaction(|tx| {
        let meta = tx
            .metas
            .get(request.diff.new.id())
            .ok_or(ClientError(DocumentNotFound))?;

        let meta_owner = meta.owner();

        let direct_access = meta_owner.0 == req_pk;

        let mut tree = ServerTree::new(meta_owner, &mut tx.owned_files, &mut tx.metas)?.to_lazy();

        let mut share_access = false;

        if !direct_access {
            for ancestor in tree
                .ancestors(request.diff.id())?
                .iter()
                .chain(vec![request.diff.new.id()])
            {
                let meta = tree.find(ancestor)?;

                if meta
                    .user_access_keys()
                    .iter()
                    .any(|access| access.encrypted_for == req_pk)
                {
                    share_access = true;
                    break;
                }
            }
        }

        if !direct_access && !share_access {
            return Err(ClientError(NotPermissioned));
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
        let meta = tx
            .metas
            .get(request.diff.new.id())
            .ok_or(ClientError(DocumentNotFound))?;

        let meta_owner = meta.owner();

        let mut tree = ServerTree::new(meta_owner, &mut tx.owned_files, &mut tx.metas)?.to_lazy();
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

    server_state.index_db.transaction(|tx| {
        let meta = tx
            .metas
            .get(&request.id)
            .ok_or(ClientError(GetDocumentError::DocumentNotFound))?;

        let meta_owner = meta.owner();

        let direct_access = meta_owner.0 == context.public_key;

        let mut tree = ServerTree::new(meta_owner, &mut tx.owned_files, &mut tx.metas)?.to_lazy();

        let mut share_access = false;

        if !direct_access {
            for ancestor in tree.ancestors(&request.id)?.iter().chain(vec![&request.id]) {
                let meta = tree.find(ancestor)?;

                if meta
                    .user_access_keys()
                    .iter()
                    .any(|access| access.encrypted_for == context.public_key)
                {
                    share_access = true;
                    break;
                }
            }
        }

        if !direct_access && !share_access {
            return Err(ClientError(GetDocumentError::NotPermissioned));
        }

        let meta = tree
            .maybe_find(&request.id)
            .ok_or(ClientError(GetDocumentError::DocumentNotFound))?;

        let hmac = meta
            .document_hmac()
            .ok_or(ClientError(GetDocumentError::DocumentNotFound))?;

        if request.hmac != *hmac {
            return Err(ClientError(GetDocumentError::DocumentNotFound));
        }

        if tree.calculate_deleted(&request.id)? {
            return Err(ClientError(GetDocumentError::DocumentNotFound));
        }

        Ok(())
    })??;

    let content = document_service::get(server_state, &request.id, &request.hmac).await?;

    Ok(GetDocumentResponse { content })
}

pub async fn get_updates(
    context: RequestContext<'_, GetUpdatesRequest>,
) -> Result<GetUpdatesResponse, ServerError<GetUpdatesError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let mut result = Vec::new();
    server_state.index_db.transaction(|tx| {
        let owners = tx
            .owned_files
            .keys()
            .iter()
            .map(|owner| **owner)
            .collect::<Vec<_>>();
        for owner in owners {
            let mut tree = ServerTree::new(owner, &mut tx.owned_files, &mut tx.metas)?.to_lazy();

            let mut result_ids = HashSet::new();
            if owner.0 == context.public_key {
                for id in tree.owned_ids() {
                    if tree.find(&id)?.version > request.since_metadata_version {
                        result_ids.insert(id);
                    }
                }
            } else {
                for id in tree.owned_ids() {
                    let file = tree.find(&id)?;
                    if file
                        .user_access_keys()
                        .iter()
                        .any(|k| k.encrypted_for == context.public_key)
                    {
                        if file.version > request.since_metadata_version {
                            result_ids.insert(id);
                            result_ids.extend(tree.descendents(&id)?);
                        } else {
                            for id in tree.descendents(&id)? {
                                if tree.find(&id)?.version > request.since_metadata_version {
                                    result_ids.insert(id);
                                }
                            }
                        }
                    }
                }
            }

            result.extend(
                tree.all_files()?
                    .iter()
                    .filter(|meta| result_ids.contains(meta.id()))
                    .map(|meta| meta.file.clone()),
            );
        }

        Ok(GetUpdatesResponse {
            as_of_metadata_version: get_time().0 as u64,
            file_metadata: result,
        })
    })?
}
