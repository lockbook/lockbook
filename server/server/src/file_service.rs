use crate::ServerError;
use crate::ServerError::ClientError;
use crate::{document_service, RequestContext};
use hmdb::transaction::Transaction;
use itertools::Itertools;
use lockbook_shared::api::*;
use lockbook_shared::clock::get_time;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{Diff, FileDiff, Owner};
use lockbook_shared::server_file::IntoServerFile;
use lockbook_shared::server_tree::ServerTree;
use lockbook_shared::tree_like::{Stagable, TreeLike};
use std::collections::{HashMap, HashSet};

pub async fn upsert_file_metadata(
    context: RequestContext<'_, UpsertRequest>,
) -> Result<(), ServerError<UpsertError>> {
    let (request, server_state) = (context.request, context.server_state);
    let req_owner = Owner(context.public_key);

    let mut tree_diff_group: HashMap<Owner, Vec<FileDiff>> = HashMap::new();
    for diff in request.updates {
        let owner = diff.new.owner();
        let mut existing = tree_diff_group.remove(&owner).unwrap_or_default();
        existing.push(diff);
        tree_diff_group.insert(owner, existing);
    }
    let mut prior_deleted_docs = HashSet::new();
    let mut new_deleted = vec![];

    let res: Result<(), ServerError<UpsertError>> =
        context.server_state.index_db.transaction(|tx| {
            // validate all trees
            for (owner, updates) in tree_diff_group.clone() {
                let mut tree =
                    ServerTree { owner, owned: &mut tx.owned_files, metas: &mut tx.metas }
                        .to_lazy();

                for id in tree.owned_ids() {
                    if tree.find(&id)?.is_document() && tree.calculate_deleted(&id)? {
                        prior_deleted_docs.insert(id);
                    }
                }

                let mut tree = tree.stage_diff(&req_owner, updates)?;
                tree.validate()?;
            }

            for (owner, updates) in tree_diff_group {
                let mut tree =
                    ServerTree { owner, owned: &mut tx.owned_files, metas: &mut tx.metas }
                        .to_lazy()
                        .stage_diff(&req_owner, updates)?
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

    let req_pk = context.public_key;

    context.server_state.index_db.transaction(|tx| {
        let meta = tx
            .metas
            .get(&request.diff.new.id())
            .ok_or(ClientError(DocumentNotFound))?;

        let meta_owner = meta.owner();

        let direct_access = meta_owner.0 == req_pk;

        let mut tree =
            ServerTree { owner: meta_owner, owned: &mut tx.owned_files, metas: &mut tx.metas }
                .to_lazy();

        let mut share_access = false;

        if !direct_access {
            for ancestor in tree.ancestors(request.diff.id())? {
                let meta = tree.find(&ancestor)?;

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

        let mut tree =
            ServerTree { owner: meta_owner, owned: &mut tx.owned_files, metas: &mut tx.metas }
                .to_lazy();

        let mut share_access = false;

        if !direct_access {
            for ancestor in tree.ancestors(&request.id)? {
                let meta = tree.find(&ancestor)?;

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
    server_state.index_db.transaction(|tx| {
        let mut file_metadata = vec![];
        let owners = tx
            .owned_files
            .keys()
            .iter()
            .map(|owner| **owner)
            .collect_vec();
        for owner in owners {
            let mut tree =
                ServerTree { owner, owned: &mut tx.owned_files, metas: &mut tx.metas }.to_lazy();
            let mut shared_files = HashSet::new();
            for id in tree.owned_ids() {
                if tree
                    .find(&id)?
                    .user_access_keys()
                    .iter()
                    .any(|k| k.encrypted_for == context.public_key)
                {
                    shared_files.extend(tree.descendents(&id)?);
                    shared_files.insert(id);
                }
            }
            let subtree_updates = tree
                .all_files()?
                .iter()
                .filter(|meta| {
                    meta.version > request.since_metadata_version
                        && shared_files.contains(meta.id())
                })
                .map(|meta| meta.file.clone())
                .collect_vec();
            file_metadata.extend(subtree_updates);
        }

        let as_of_metadata_version = get_time().0 as u64;
        Ok(GetUpdatesResponse { as_of_metadata_version, file_metadata })
    })?
}
