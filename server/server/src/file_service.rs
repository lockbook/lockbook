use crate::account_service::is_admin;
use crate::ServerError::ClientError;
use crate::{document_service, RequestContext, ServerState};
use crate::{ServerError, Tx};
use hmdb::transaction::Transaction;
use lockbook_shared::api::*;
use lockbook_shared::clock::get_time;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{Diff, Owner};
use lockbook_shared::server_file::IntoServerFile;
use lockbook_shared::server_tree::ServerTree;
use lockbook_shared::tree_like::{Stagable, TreeLike};
use lockbook_shared::{SharedError, SharedResult};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use tracing::{debug, error, warn};

pub async fn upsert_file_metadata(
    context: RequestContext<'_, UpsertRequest>,
) -> Result<(), ServerError<UpsertError>> {
    let (request, server_state) = (context.request, context.server_state);
    let req_owner = Owner(context.public_key);

    let mut prior_deleted_docs = HashSet::new();
    let mut new_deleted = vec![];

    context
        .server_state
        .index_db
        .transaction::<_, Result<(), ServerError<_>>>(|tx| {
            let mut tree = ServerTree::new(
                req_owner,
                &mut tx.owned_files,
                &mut tx.shared_files,
                &mut tx.file_children,
                &mut tx.metas,
            )?
            .to_lazy();

            for id in tree.owned_ids() {
                if tree.find(&id)?.is_document() && tree.calculate_deleted(&id)? {
                    prior_deleted_docs.insert(id);
                }
            }

            let mut tree = tree
                .stage_diff(request.updates.clone())?
                .validate(req_owner)?
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
                        debug!("deleting id: {}", *meta.id());
                    }
                }
            }
            Ok(())
        })??;

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
    let owner = Owner(context.public_key);

    // Validate Diff
    if request.diff.diff() != vec![Diff::Hmac] {
        return Err(ClientError(DiffMalformed));
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

        let mut tree = ServerTree::new(
            owner,
            &mut tx.owned_files,
            &mut tx.shared_files,
            &mut tx.file_children,
            &mut tx.metas,
        )?
        .to_lazy();

        if tree.maybe_find(request.diff.new.id()).is_none() {
            return Err(ClientError(NotPermissioned));
        }

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
    debug!("Updating document: {}", request.diff.new.id());
    document_service::insert(
        server_state,
        request.diff.new.id(),
        request.diff.new.document_hmac().unwrap(),
        &request.new_content,
    )
    .await?;

    let result = server_state.index_db.transaction(|tx| {
        let mut tree = ServerTree::new(
            owner,
            &mut tx.owned_files,
            &mut tx.shared_files,
            &mut tx.file_children,
            &mut tx.metas,
        )?
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
        let meta_exists = tx.metas.get(&request.id).is_some();

        let mut tree = ServerTree::new(
            Owner(context.public_key),
            &mut tx.owned_files,
            &mut tx.shared_files,
            &mut tx.file_children,
            &mut tx.metas,
        )?
        .to_lazy();

        let meta = match tree.maybe_find(&request.id) {
            Some(meta) => Ok(meta),
            None => Err(if meta_exists {
                ClientError(GetDocumentError::NotPermissioned)
            } else {
                ClientError(GetDocumentError::DocumentNotFound)
            }),
        }?;

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
    let owner = Owner(context.public_key);
    server_state.index_db.transaction(|tx| {
        let mut tree = ServerTree::new(
            owner,
            &mut tx.owned_files,
            &mut tx.shared_files,
            &mut tx.file_children,
            &mut tx.metas,
        )?
        .to_lazy();

        let mut result_ids = HashSet::new();
        for id in tree.owned_ids() {
            let file = tree.find(&id)?;
            if file.version > request.since_metadata_version {
                result_ids.insert(id);
                if file.owner() != owner
                    && file
                        .user_access_keys()
                        .iter()
                        .any(|k| k.encrypted_for == context.public_key)
                {
                    result_ids.insert(id);
                    result_ids.extend(tree.descendants(&id)?);
                }
            }
        }

        Ok(GetUpdatesResponse {
            as_of_metadata_version: get_time().0 as u64,
            file_metadata: tree
                .all_files()?
                .iter()
                .filter(|meta| result_ids.contains(meta.id()))
                .map(|meta| meta.file.clone())
                .collect(),
        })
    })?
}

pub async fn admin_disappear_file(
    context: RequestContext<'_, AdminDisappearFileRequest>,
) -> Result<(), ServerError<AdminDisappearFileError>> {
    let db = &context.server_state.index_db;
    if !is_admin::<AdminDisappearFileError>(
        db,
        &context.public_key,
        &context.server_state.config.admin.admins,
    )? {
        return Err(ClientError(AdminDisappearFileError::NotPermissioned));
    }

    let username = db
        .accounts
        .get(&Owner(context.public_key))?
        .map(|account| account.username)
        .unwrap_or_else(|| "~unknown~".to_string());

    warn!("admin: {} disappeared file {}", username, context.request.id);

    context
        .server_state
        .index_db
        .transaction::<_, Result<(), ServerError<_>>>(|tx| {
            let meta = tx
                .metas
                .delete(context.request.id)
                .ok_or(ClientError(AdminDisappearFileError::FileNonexistent))?;

            // maintain index: owned_files
            let owner = meta.owner();
            let mut owned_files = tx.owned_files.delete(owner).ok_or_else(|| {
                internal!(
                    "Attempted to disappear a file, the owner was not present, id: {}, owner: {:?}",
                    context.request.id,
                    owner
                )
            })?;
            if !owned_files.remove(&context.request.id) {
                error!(
                    "attempted to disappear a file, the owner didn't own it, id: {}, owner: {:?}",
                    context.request.id, owner
                );
            }
            tx.owned_files.insert(owner, owned_files);

            // maintain index: shared_files
            for user_access_key in meta.user_access_keys() {
                let sharee = Owner(user_access_key.encrypted_for);
                let mut shared_files = tx.shared_files.delete(sharee).ok_or_else(|| {
                    internal!(
                        "Attempted to disappear a file, the sharee was not present, id: {}, sharee: {:?}",
                        context.request.id,
                        sharee
                    )
                })?;
                if !shared_files.remove(&context.request.id) {
                    error!(
                        "attempted to disappear a file, a sharee didn't have it shared, id: {}, sharee: {:?}",
                        context.request.id, sharee
                    );
                }
                tx.shared_files.insert(sharee, shared_files);
            }

            // maintain index: file_children
            let mut file_children = tx.file_children.delete(*meta.parent()).ok_or_else(|| {
                internal!(
                    "Attempted to disappear a file, the parent was not present, id: {}, parent: {:?}",
                    context.request.id,
                    meta.parent()
                )
            })?;
            if !file_children.remove(&context.request.id) {
                error!(
                    "attempted to disappear a file, the parent didn't have it as a child, id: {}, parent: {:?}",
                    context.request.id, meta.parent()
                );
            }
            tx.file_children.insert(*meta.parent(), file_children);

            Ok(())
        })??;

    Ok(())
}

pub async fn admin_validate_account(
    context: RequestContext<'_, AdminValidateAccountRequest>,
) -> Result<AdminValidateAccount, ServerError<AdminValidateAccountError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let db = &server_state.index_db;
    if !is_admin::<AdminValidateAccountError>(
        db,
        &context.public_key,
        &context.server_state.config.admin.admins,
    )? {
        return Err(ClientError(AdminValidateAccountError::NotPermissioned));
    }

    let result: Result<AdminValidateAccount, ServerError<AdminValidateAccountError>> =
        server_state.index_db.transaction(|tx| {
            let owner = *tx
                .usernames
                .get(&request.username)
                .ok_or(ClientError(AdminValidateAccountError::UserNotFound))?;

            Ok(validate_account_helper(tx, owner, server_state)?)
        })?;

    result
}

pub fn validate_account_helper(
    tx: &mut Tx<'_>, owner: Owner, server_state: &ServerState,
) -> SharedResult<AdminValidateAccount> {
    let mut result = AdminValidateAccount::default();

    let mut tree = ServerTree::new(
        owner,
        &mut tx.owned_files,
        &mut tx.shared_files,
        &mut tx.file_children,
        &mut tx.metas,
    )?
    .to_lazy();

    for id in tree.owned_ids() {
        if !tree.calculate_deleted(&id)? {
            let file = tree.find(&id)?;
            if file.is_document() && file.document_hmac().is_some() {
                if tx.sizes.get(&id).is_none() {
                    result.documents_missing_size.push(id);
                }

                if !document_service::exists(server_state, &id, file.document_hmac().unwrap()) {
                    result.documents_missing_content.push(id);
                }
            }
        }
    }

    let validation_res = tree.stage(None).validate(owner);
    match validation_res {
        Ok(_) => {}
        Err(SharedError::ValidationFailure(validation)) => {
            result.tree_validation_failures.push(validation)
        }
        Err(err) => {
            error!("Unexpected error while validating {:?}'s tree: {:?}", owner, err)
        }
    }

    Ok(result)
}

pub async fn admin_validate_server(
    context: RequestContext<'_, AdminValidateServerRequest>,
) -> Result<AdminValidateServer, ServerError<AdminValidateServerError>> {
    if !is_admin::<AdminValidateServerError>(
        &context.server_state.index_db,
        &context.public_key,
        &context.server_state.config.admin.admins,
    )? {
        return Err(ClientError(AdminValidateServerError::NotPermissioned));
    }

    let mut result: AdminValidateServer = Default::default();

    context
        .server_state
        .index_db
        .transaction::<_, Result<(), ServerError<_>>>(|tx| {
            let mut deleted_ids = HashSet::new();
            for (id, meta) in tx.metas.get_all().clone() {
                // todo: optimize
                let mut tree = ServerTree::new(
                    meta.owner(),
                    &mut tx.owned_files,
                    &mut tx.shared_files,
                    &mut tx.file_children,
                    &mut tx.metas,
                )?
                .to_lazy();
                if tree.calculate_deleted(&id)? {
                    deleted_ids.insert(id);
                }
            }

            // validate accounts
            for (owner, account) in tx.accounts.get_all().clone() {
                let validation = validate_account_helper(tx, owner, context.server_state)?;
                if !validation.is_empty() {
                    result
                        .users_with_validation_failures
                        .insert(account.username, validation);
                }
            }

            // validate index: usernames
            for (username, owner) in tx.usernames.get_all().clone() {
                if let Some(account) = tx.accounts.get(&owner) {
                    if username != account.username {
                        result
                            .usernames_mapped_to_wrong_accounts
                            .insert(username, account.username.clone());
                    }
                } else {
                    result
                        .usernames_mapped_to_nonexistent_accounts
                        .insert(username, owner);
                }
            }
            for (_, account) in tx.accounts.get_all().clone() {
                if tx.usernames.get(&account.username).is_none() {
                    result
                        .usernames_unmapped_to_accounts
                        .insert(account.username.clone());
                }
            }

            // validate index: owned_files
            for (owner, ids) in tx.owned_files.get_all().clone() {
                for id in ids {
                    if let Some(meta) = tx.metas.get(&id) {
                        if meta.owner() != owner {
                            insert(&mut result.owners_mapped_to_unowned_files, owner, id);
                        }
                    } else {
                        insert(&mut result.owners_mapped_to_nonexistent_files, owner, id);
                    }
                }
            }
            for (id, meta) in tx.metas.get_all().clone() {
                if let Some(ids) = tx.owned_files.get(&meta.owner()) {
                    if !ids.contains(&id) {
                        insert(
                            &mut result.owners_unmapped_to_owned_files,
                            meta.owner(),
                            *meta.id(),
                        );
                    }
                } else {
                    result.owners_unmapped.insert(meta.owner());
                }
            }

            // validate index: shared_files
            for (sharee, ids) in tx.shared_files.get_all().clone() {
                for id in ids {
                    if let Some(meta) = tx.metas.get(&id) {
                        if !meta
                            .user_access_keys()
                            .iter()
                            .any(|k| k.encrypted_for == sharee.0 && k.encrypted_by != sharee.0)
                        {
                            insert(&mut result.sharees_mapped_to_unshared_files, sharee, id);
                        }
                    } else {
                        insert(&mut result.sharees_mapped_to_nonexistent_files, sharee, id);
                    }
                }
            }
            for (id, meta) in tx.metas.get_all().clone() {
                for k in meta.user_access_keys() {
                    let sharee = Owner(k.encrypted_for);
                    if let Some(ids) = tx.shared_files.get(&sharee) {
                        let self_share = k.encrypted_for == k.encrypted_by;
                        let indexed_share = ids.contains(&id);
                        if self_share && indexed_share {
                            insert(&mut result.sharees_mapped_for_owned_files, sharee, id);
                        } else if !self_share && !indexed_share {
                            insert(&mut result.sharees_unmapped_to_shared_files, sharee, id);
                        }
                    } else {
                        result.sharees_unmapped.insert(meta.owner());
                    }
                }
            }

            // validate index: file_children
            for (parent_id, child_ids) in tx.file_children.get_all().clone() {
                for child_id in child_ids {
                    if let Some(meta) = tx.metas.get(&child_id) {
                        if meta.parent() != &parent_id {
                            insert(
                                &mut result.files_mapped_as_parent_to_non_children,
                                parent_id,
                                child_id,
                            );
                        }
                    } else {
                        insert(
                            &mut result.files_mapped_as_parent_to_nonexistent_children,
                            parent_id,
                            child_id,
                        );
                    }
                }
            }
            for (id, meta) in tx.metas.get_all().clone() {
                if let Some(child_ids) = tx.file_children.get(meta.parent()) {
                    if meta.is_root() && child_ids.contains(&id) {
                        result.files_mapped_as_parent_to_self.insert(id);
                    } else if !meta.is_root() && !child_ids.contains(&id) {
                        insert(
                            &mut result.files_unmapped_as_parent_to_children,
                            *meta.parent(),
                            id,
                        );
                    }
                } else {
                    result.files_unmapped_as_parent.insert(*meta.parent());
                }
            }

            // validate index: sizes (todo: validate size values)
            for (id, _) in tx.sizes.get_all().clone() {
                if let Some(meta) = tx.metas.get(&id) {
                    if meta.document_hmac().is_none() {
                        result.sizes_mapped_for_files_without_hmac.insert(id);
                    }
                } else {
                    result.sizes_mapped_for_nonexistent_files.insert(id);
                }
            }
            for (id, meta) in tx.metas.get_all().clone() {
                if !deleted_ids.contains(&id)
                    && meta.document_hmac().is_some()
                    && tx.sizes.get(&id).is_none()
                {
                    result.sizes_unmapped_for_files_with_hmac.insert(id);
                }
            }

            // validate presence of documents
            for (id, meta) in tx.metas.get_all().clone() {
                if let Some(hmac) = meta.document_hmac() {
                    if !deleted_ids.contains(&id)
                        && !document_service::exists(context.server_state, &id, hmac)
                    {
                        result.files_with_hmacs_and_no_contents.insert(id);
                    }
                }
            }

            Ok(())
        })??;

    Ok(result)
}

fn insert<K: Hash + Eq, V: Hash + Eq>(map: &mut HashMap<K, HashSet<V>>, k: K, v: V) {
    map.entry(k).or_insert_with(Default::default).insert(v);
}
