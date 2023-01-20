use crate::account_service::is_admin;
use crate::ServerError::ClientError;
use crate::{document_service, RequestContext, ServerState};
use crate::{ServerError, Tx};
use hmdb::transaction::Transaction;
use lockbook_shared::api::*;
use lockbook_shared::clock::get_time;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{Diff, DocumentHmac, Owner};
use lockbook_shared::server_file::IntoServerFile;
use lockbook_shared::server_tree::ServerTree;
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::{SharedError, SharedResult};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use tracing::{debug, error, warn};
use uuid::Uuid;

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

            let mut tree = tree.stage_diff(request.updates.clone())?;
            tree.validate(req_owner)?;
            let mut tree = tree.promote();

            for id in tree.owned_ids() {
                if tree.find(&id)?.is_document()
                    && tree.calculate_deleted(&id)?
                    && !prior_deleted_docs.contains(&id)
                {
                    let meta = tree.find(&id)?;
                    if let Some(hmac) = meta.file.timestamped_value.value.document_hmac {
                        tx.sizes.delete(*meta.id());
                        new_deleted.push((*meta.id(), hmac));
                    }
                }
            }

            tx.last_seen.insert(req_owner, get_time().0 as u64);
            Ok(())
        })??;

    for update in request.updates {
        let new = update.new;
        let id = *new.id();
        match update.old {
            None => {
                debug!(?id, "Created file");
            }
            Some(old) => {
                let old_parent = *old.parent();
                let new_parent = *new.parent();
                if old.parent() != new.parent() {
                    debug!(?id, ?old_parent, ?new_parent, "Moved file");
                }
                if old.secret_name() != new.secret_name() {
                    debug!(?id, "Renamed file");
                }
                if old.owner() != new.owner() {
                    debug!(?id, ?old_parent, ?new_parent, "Changed owner for file");
                }
                if old.explicitly_deleted() != new.explicitly_deleted() {
                    debug!(?id, "Deleted file");
                }
                if old.user_access_keys() != new.user_access_keys() {
                    let all_sharees: Vec<_> = old
                        .user_access_keys()
                        .iter()
                        .chain(new.user_access_keys().iter())
                        .map(|k| Owner(k.encrypted_for))
                        .collect();
                    for sharee in all_sharees {
                        let new = if let Some(k) = new
                            .user_access_keys()
                            .iter()
                            .find(|k| k.encrypted_for == sharee.0)
                        {
                            k
                        } else {
                            debug!(?id, ?sharee, "Disappeared user access key");
                            continue;
                        };
                        let old = if let Some(k) = old
                            .user_access_keys()
                            .iter()
                            .find(|k| k.encrypted_for == sharee.0)
                        {
                            k
                        } else {
                            debug!(?id, ?sharee, ?new.mode, "Added user access key");
                            continue;
                        };
                        if old.mode != new.mode {
                            debug!(?id, ?sharee, ?old.mode, ?new.mode, "Modified user access mode");
                        }
                        if old.deleted != new.deleted {
                            debug!(?id, ?sharee, ?old.deleted, ?new.deleted, "Deleted user access key");
                        }
                    }
                }
            }
        }
    }

    for (id, hmac) in new_deleted {
        document_service::delete(server_state, &id, &hmac).await?;
        let hmac = base64::encode_config(hmac, base64::URL_SAFE);
        debug!(?id, ?hmac, "Deleted document contents");
    }
    Ok(())
}

pub async fn change_doc(
    context: RequestContext<'_, ChangeDocRequest>,
) -> Result<(), ServerError<ChangeDocError>> {
    use ChangeDocError::*;

    let (request, server_state) = (context.request, context.server_state);
    let owner = Owner(context.public_key);
    let id = *request.diff.id();

    // Validate Diff
    if request.diff.diff() != vec![Diff::Hmac] {
        return Err(ClientError(DiffMalformed));
    }

    let hmac = if let Some(hmac) = request.diff.new.document_hmac() {
        base64::encode_config(hmac, base64::URL_SAFE)
    } else {
        return Err(ClientError(HmacMissing));
    };

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
    document_service::insert(
        server_state,
        request.diff.new.id(),
        request.diff.new.document_hmac().unwrap(),
        &request.new_content,
    )
    .await?;
    debug!(?id, ?hmac, "Inserted document contents");

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
        tx.last_seen.insert(owner, get_time().0 as u64);

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
        debug!(?id, ?hmac, "Cleaned up new document contents after failed metadata update");
    }

    result?;

    // New
    if let Some(hmac) = request.diff.old.unwrap().document_hmac() {
        document_service::delete(server_state, request.diff.new.id(), hmac).await?;
        let old_hmac = base64::encode_config(hmac, base64::URL_SAFE);
        debug!(?id, ?old_hmac, "Cleaned up old document contents after successful metadata update");
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

pub async fn get_file_ids(
    context: RequestContext<'_, GetFileIdsRequest>,
) -> Result<GetFileIdsResponse, ServerError<GetFileIdsError>> {
    let owner = Owner(context.public_key);
    context.server_state.index_db.transaction(|tx| {
        Ok(GetFileIdsResponse {
            ids: ServerTree::new(
                owner,
                &mut tx.owned_files,
                &mut tx.shared_files,
                &mut tx.file_children,
                &mut tx.metas,
            )?
            .owned_ids(),
        })
    })?
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
            if file.version >= request.since_metadata_version {
                result_ids.insert(id);
                if file.owner() != owner
                    && file
                        .user_access_keys()
                        .iter()
                        .any(|k| !k.deleted && k.encrypted_for == context.public_key)
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

    let docs_to_delete: Result<Vec<(Uuid, DocumentHmac)>, ServerError<AdminDisappearFileError>> = context
        .server_state
        .index_db
        .transaction(|tx| {
            let owner = {
                let meta = tx
                    .metas
                    .get(&context.request.id)
                    .ok_or(ClientError(AdminDisappearFileError::FileNonexistent))?.clone();
                if meta.is_root() {
                    return Err(ClientError(AdminDisappearFileError::RootModificationInvalid));
                }
                meta.owner()
            };
            let mut tree = ServerTree::new(
                owner,
                &mut tx.owned_files,
                &mut tx.shared_files,
                &mut tx.file_children,
                &mut tx.metas,
            )?.to_lazy();

            let mut docs_to_delete = Vec::new();
            let metas_to_delete = {
                let mut metas_to_delete = tree.descendants(&context.request.id)?;
                metas_to_delete.insert(context.request.id);
                metas_to_delete
            };
            for id in metas_to_delete.clone() {
                if !tree.calculate_deleted(&id)? {
                    let meta = tree.find(&id)?;
                    if meta.is_document() && meta.owner() == owner {
                        if let Some(hmac) = meta.document_hmac() {
                            docs_to_delete.push((*meta.id(), *hmac));
                            tx.sizes.delete(id);
                        }
                    }
                }
            }

            for id in metas_to_delete {
                let meta = tx
                    .metas
                    .delete(id)
                    .ok_or(ClientError(AdminDisappearFileError::FileNonexistent))?;

                // maintain index: owned_files
                let owner = meta.owner();
                if let Some(mut owned_files) = tx.owned_files.delete(owner) {
                    if !owned_files.remove(&id) {
                        error!(?id, ?owner, "attempted to disappear a file, the owner didn't own it");
                    }
                    tx.owned_files.insert(owner, owned_files);
                } else {
                    error!(
                        "attempted to disappear a file, the owner was not present, id: {}, owner: {:?}",
                        id,
                        owner
                    );
                }

                // maintain index: shared_files
                for user_access_key in meta.user_access_keys() {
                    let sharee = Owner(user_access_key.encrypted_for);
                    if let Some(mut shared_files) = tx.shared_files.delete(sharee) {
                        if !shared_files.remove(&id) {
                            error!(?id, ?sharee, "attempted to disappear a file, a sharee didn't have it shared");
                        }
                        tx.shared_files.insert(sharee, shared_files);
                    } else {
                        error!(
                            "attempted to disappear a file, the sharee was not present, id: {}, sharee: {:?}",
                            id,
                            sharee
                        );
                    }
                }

                // maintain index: file_children
                let parent = *meta.parent();
                if let Some(mut file_children) = tx.file_children.delete(*meta.parent()) {
                    if !file_children.remove(&id) {
                        error!(?id, ?parent, "attempted to disappear a file, the parent didn't have it as a child");
                    }
                    tx.file_children.insert(*meta.parent(), file_children);
                } else {
                    error!(
                        "attempted to disappear a file, the parent was not present, id: {}, parent: {:?}",
                        id,
                        meta.parent()
                    );
                }
            }

            Ok(docs_to_delete)
        })?;

    for (id, version) in docs_to_delete? {
        document_service::delete(context.server_state, &id, &version).await?;
    }

    let username = db
        .accounts
        .get(&Owner(context.public_key))?
        .map(|account| account.username)
        .unwrap_or_else(|| "~unknown~".to_string());
    warn!(?username, ?context.request.id, "Disappeared file");

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
            error!(?owner, ?err, "Unexpected error while validating tree")
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
                        if !meta.user_access_keys().iter().any(|k| {
                            !k.deleted && k.encrypted_for == sharee.0 && k.encrypted_by != sharee.0
                        }) {
                            insert(&mut result.sharees_mapped_to_unshared_files, sharee, id);
                        }
                    } else {
                        insert(&mut result.sharees_mapped_to_nonexistent_files, sharee, id);
                    }
                }
            }
            for (id, meta) in tx.metas.get_all().clone() {
                for k in meta.user_access_keys() {
                    if k.deleted {
                        continue;
                    }
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

pub async fn admin_file_info(
    context: RequestContext<'_, AdminFileInfoRequest>,
) -> Result<AdminFileInfoResponse, ServerError<AdminFileInfoError>> {
    let (request, server_state) = (&context.request, context.server_state);
    let db = &server_state.index_db;
    if !is_admin::<AdminFileInfoError>(
        db,
        &context.public_key,
        &context.server_state.config.admin.admins,
    )? {
        return Err(ClientError(AdminFileInfoError::NotPermissioned));
    }

    server_state
        .index_db
        .transaction::<_, Result<AdminFileInfoResponse, ServerError<_>>>(|tx| {
            let file = tx
                .metas
                .get(&request.id)
                .ok_or(ClientError(AdminFileInfoError::FileNonexistent))?
                .clone();

            let mut tree = ServerTree::new(
                file.owner(),
                &mut tx.owned_files,
                &mut tx.shared_files,
                &mut tx.file_children,
                &mut tx.metas,
            )?
            .to_lazy();

            let ancestors = tree
                .ancestors(&request.id)?
                .into_iter()
                .filter_map(|id| tree.maybe_find(&id))
                .cloned()
                .collect();
            let descendants = tree
                .descendants(&request.id)?
                .into_iter()
                .filter_map(|id| tree.maybe_find(&id))
                .cloned()
                .collect();

            Ok(AdminFileInfoResponse { file, ancestors, descendants })
        })?
}

pub async fn admin_rebuild_index(
    context: RequestContext<'_, AdminRebuildIndexRequest>,
) -> Result<(), ServerError<AdminRebuildIndexError>> {
    context
        .server_state
        .index_db
        .transaction(|tx| match context.request.index {
            ServerIndex::OwnedFiles => {
                let mut owned_files = HashMap::new();
                for owner in tx.accounts.keys() {
                    owned_files.insert(*owner, HashSet::new());
                }
                for (id, file) in tx.metas.get_all() {
                    if let Some(owned_files) = owned_files.get_mut(&file.owner()) {
                        owned_files.insert(*id);
                    }
                }

                tx.owned_files.clear();
                for (k, v) in owned_files {
                    tx.owned_files.insert(k, v);
                }
                Ok(())
            }
            ServerIndex::SharedFiles => {
                let mut shared_files = HashMap::new();
                for owner in tx.accounts.keys() {
                    shared_files.insert(*owner, HashSet::new());
                }
                for (id, file) in tx.metas.get_all() {
                    for user_access_key in file.user_access_keys() {
                        if user_access_key.encrypted_for != user_access_key.encrypted_by {
                            if let Some(shared_files) =
                                shared_files.get_mut(&Owner(user_access_key.encrypted_for))
                            {
                                shared_files.insert(*id);
                            }
                        }
                    }
                }

                tx.shared_files.clear();
                for (k, v) in shared_files {
                    tx.shared_files.insert(k, v);
                }
                Ok(())
            }
            ServerIndex::FileChildren => {
                let mut file_children = HashMap::new();
                for id in tx.metas.keys() {
                    file_children.insert(*id, HashSet::new());
                }
                for (id, file) in tx.metas.get_all() {
                    if let Some(file_children) = file_children.get_mut(file.parent()) {
                        file_children.insert(*id);
                    }
                }

                tx.file_children.clear();
                for (k, v) in file_children {
                    tx.file_children.insert(k, v);
                }
                Ok(())
            }
        })?
}
