use crate::ServerError;
use crate::ServerError::ClientError;
use crate::billing::app_store_client::AppStoreClient;
use crate::billing::google_play_client::GooglePlayClient;
use crate::billing::stripe_client::StripeClient;
use crate::document_service::DocumentService;
use crate::schema::ServerDb;

use crate::{RequestContext, ServerState};
use db_rs::Db;
use lb_rs::model::api::{UpsertError, *};
use lb_rs::model::clock::get_time;
use lb_rs::model::crypto::Timestamped;
use lb_rs::model::errors::{LbErrKind, LbResult};
use lb_rs::model::file_like::FileLike;
use lb_rs::model::file_metadata::{Diff, FileDiff, FileMetadata, Owner};
use lb_rs::model::meta::Meta;
use lb_rs::model::server_meta::{IntoServerMeta, ServerMeta};
use lb_rs::model::server_tree::ServerTree;
use lb_rs::model::signed_file::SignedFile;
use lb_rs::model::tree_like::TreeLike;
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::ops::DerefMut;
use tracing::{debug, error, warn};

impl<S, A, G, D> ServerState<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    pub async fn upsert_file_metadata(
        &self, context: RequestContext<UpsertRequest>,
    ) -> Result<(), ServerError<UpsertError>> {
        let request = context.request;
        let req_owner = Owner(context.public_key);

        let mut new_deleted = vec![];
        {
            let mut prior_deleted = HashSet::new();
            let mut current_deleted = HashSet::new();

            let mut lock = self.index_db.lock().await;
            let db = lock.deref_mut();
            let tx = db.begin_transaction()?;

            let usage_cap =
                Self::get_cap(db, &context.public_key).map_err(|err| internal!("{:?}", err))?;

            let mut tree = ServerTree::new(
                req_owner,
                &mut db.owned_files,
                &mut db.shared_files,
                &mut db.file_children,
                &mut db.metas,
            )?
            .to_lazy();

            let old_usage = Self::get_usage_helper(&mut tree)
                .map_err(|err| internal!("{:?}", err))?
                .iter()
                .map(|f| f.size_bytes)
                .sum::<u64>();

            for id in tree.ids() {
                if tree.calculate_deleted(&id)? {
                    prior_deleted.insert(id);
                }
            }

            let mut tree = tree.stage_diff(request.updates.clone())?;
            for id in tree.ids() {
                if tree.calculate_deleted(&id)? {
                    current_deleted.insert(id);
                }
            }

            tree.validate(req_owner)?;

            let new_usage = Self::get_usage_helper(&mut tree)
                .map_err(|err| internal!("{:?}", err))?
                .iter()
                .map(|f| f.size_bytes)
                .sum::<u64>();

            debug!(?old_usage, ?new_usage, ?usage_cap, "usage caps on upsert");

            if new_usage > usage_cap && new_usage >= old_usage {
                return Err(ClientError(UpsertError::UsageIsOverDataCap));
            }

            let tree = tree.promote()?;

            for id in tree.ids() {
                if tree.find(&id)?.is_document()
                    && current_deleted.contains(&id)
                    && !prior_deleted.contains(&id)
                {
                    let meta = tree.find(&id)?;
                    if let Some(hmac) = meta.file.timestamped_value.value.document_hmac().copied() {
                        new_deleted.push((*meta.id(), hmac));
                    }
                }
            }

            let all_files: Vec<ServerMeta> = tree.all_files()?.into_iter().cloned().collect();
            for meta in all_files {
                let id = meta.id();
                if current_deleted.contains(id) && !prior_deleted.contains(id) {
                    for user_access_info in meta.user_access_keys() {
                        db.shared_files
                            .remove(&Owner(user_access_info.encrypted_for), id)?;
                    }
                }
            }

            db.last_seen.insert(req_owner, get_time().0 as u64)?;

            tx.drop_safely()?;
        }

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
            self.document_service.delete(&id, &hmac).await?;
            let hmac = base64::encode_config(hmac, base64::URL_SAFE);
            debug!(?id, ?hmac, "Deleted document contents");
        }
        Ok(())
    }

    pub async fn change_doc(
        &self, context: RequestContext<ChangeDocRequest>,
    ) -> Result<(), ServerError<ChangeDocError>> {
        use ChangeDocError::*;

        let request = context.request;
        let mut request = ChangeDocRequestV2 {
            diff: FileDiff {
                old: request.diff.old.map(|f| f.into()),
                new: request.diff.new.into(),
            },
            new_content: request.new_content,
        };
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

        {
            let mut lock = self.index_db.lock().await;
            let db = lock.deref_mut();
            let usage_cap = Self::get_cap(db, &context.public_key)
                .map_err(|err| internal!("{:?}", err))? as usize;

            let meta = db
                .metas
                .get()
                .get(request.diff.new.id())
                .ok_or(ClientError(DocumentNotFound))?
                .clone();

            let mut tree = ServerTree::new(
                owner,
                &mut db.owned_files,
                &mut db.shared_files,
                &mut db.file_children,
                &mut db.metas,
            )?
            .to_lazy();

            let old_usage = Self::get_usage_helper(&mut tree)
                .map_err(|err| internal!("{:?}", err))?
                .iter()
                .map(|f| f.size_bytes)
                .sum::<u64>() as usize;
            let old_size = *meta.file.timestamped_value.value.doc_size();

            // populate sizes in request
            if let Some(old) = &mut request.diff.old {
                match &mut old.timestamped_value.value {
                    Meta::V1 { doc_size, .. } => {
                        *doc_size = old_size;
                    }
                }
            }

            match &mut request.diff.new.timestamped_value.value {
                Meta::V1 { doc_size, .. } => {
                    *doc_size = Some(request.new_content.value.len());
                }
            }

            let new_size = request.new_content.value.len();

            let new_usage = old_usage - old_size.unwrap_or_default() + new_size;

            debug!(?old_usage, ?new_usage, ?usage_cap, "usage caps on change doc");

            if new_usage > usage_cap {
                return Err(ClientError(UsageIsOverDataCap));
            }

            let meta_owner = meta.owner();

            let direct_access = meta_owner.0 == req_pk;

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
        }

        let new_version = get_time().0 as u64;
        let new = request.diff.new.clone().add_time(new_version);
        self.document_service
            .insert(
                request.diff.new.id(),
                request.diff.new.document_hmac().unwrap(),
                &request.new_content,
            )
            .await?;
        debug!(?id, ?hmac, "Inserted document contents");

        let result = async {
            let mut lock = self.index_db.lock().await;
            let db = lock.deref_mut();
            let tx = db.begin_transaction()?;

            let mut tree = ServerTree::new(
                owner,
                &mut db.owned_files,
                &mut db.shared_files,
                &mut db.file_children,
                &mut db.metas,
            )?
            .to_lazy();

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

            tree.stage(vec![new]).promote()?;
            db.last_seen.insert(owner, get_time().0 as u64)?;

            tx.drop_safely()?;
            drop(lock);
            Ok(())
        };

        let result = result.await;

        if result.is_err() {
            // Cleanup the NEW file created if, for some reason, the tx failed
            self.document_service
                .delete(request.diff.new.id(), request.diff.new.document_hmac().unwrap())
                .await?;
            debug!(?id, ?hmac, "Cleaned up new document contents after failed metadata update");
        }

        result?;

        // New
        if let Some(hmac) = request.diff.old.unwrap().document_hmac() {
            self.document_service
                .delete(request.diff.new.id(), hmac)
                .await?;
            let old_hmac = base64::encode_config(hmac, base64::URL_SAFE);
            debug!(
                ?id,
                ?old_hmac,
                "Cleaned up old document contents after successful metadata update"
            );
        }

        Ok(())
    }

    pub async fn get_document(
        &self, context: RequestContext<GetDocRequest>,
    ) -> Result<GetDocumentResponse, ServerError<GetDocumentError>> {
        let request = &context.request;
        {
            let mut lock = self.index_db.lock().await;
            let db = lock.deref_mut();
            let tx = db.begin_transaction()?;

            let meta_exists = db.metas.get().get(&request.id).is_some();

            let mut tree = ServerTree::new(
                Owner(context.public_key),
                &mut db.owned_files,
                &mut db.shared_files,
                &mut db.file_children,
                &mut db.metas,
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

            tx.drop_safely()?;
        }

        let content = self
            .document_service
            .get(&request.id, &request.hmac)
            .await?;
        Ok(GetDocumentResponse { content })
    }

    pub async fn get_file_ids(
        &self, context: RequestContext<GetFileIdsRequest>,
    ) -> Result<GetFileIdsResponse, ServerError<GetFileIdsError>> {
        let owner = Owner(context.public_key);
        let mut db = self.index_db.lock().await;
        let db = db.deref_mut();

        Ok(GetFileIdsResponse {
            ids: ServerTree::new(
                owner,
                &mut db.owned_files,
                &mut db.shared_files,
                &mut db.file_children,
                &mut db.metas,
            )?
            .ids()
            .into_iter()
            .collect(),
        })
    }

    pub async fn get_updates(
        &self, context: RequestContext<GetUpdatesRequest>,
    ) -> Result<GetUpdatesResponse, ServerError<GetUpdatesError>> {
        let request = &context.request;
        let owner = Owner(context.public_key);

        let mut db = self.index_db.lock().await;
        let db = db.deref_mut();
        let mut tree = ServerTree::new(
            owner,
            &mut db.owned_files,
            &mut db.shared_files,
            &mut db.file_children,
            &mut db.metas,
        )?
        .to_lazy();

        let mut result_ids = HashSet::new();
        for id in tree.ids() {
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
                .map(|meta| match meta.file.timestamped_value.value.clone() {
                    Meta::V1 {
                        id,
                        file_type,
                        parent,
                        name,
                        owner,
                        is_deleted,
                        doc_size: _,
                        doc_hmac,
                        user_access_keys,
                        folder_access_key,
                    } => SignedFile {
                        timestamped_value: Timestamped {
                            timestamp: meta.file.timestamped_value.timestamp,
                            value: FileMetadata {
                                id,
                                file_type,
                                parent,
                                name,
                                owner,
                                is_deleted,
                                document_hmac: doc_hmac,
                                user_access_keys,
                                folder_access_key,
                            },
                        },
                        signature: meta.file.signature.clone(),
                        public_key: meta.file.public_key,
                    },
                })
                .collect(),
        })
    }

    pub async fn admin_disappear_file(
        &self, context: RequestContext<AdminDisappearFileRequest>,
    ) -> Result<(), ServerError<AdminDisappearFileError>> {
        let mut docs_to_delete = Vec::new();

        {
            let mut db = self.index_db.lock().await;
            let db = db.deref_mut();
            let tx = db.begin_transaction()?;

            if !Self::is_admin::<AdminDisappearFileError>(
                db,
                &context.public_key,
                &self.config.admin.admins,
            )? {
                return Err(ClientError(AdminDisappearFileError::NotPermissioned));
            }

            let owner = {
                let meta = db
                    .metas
                    .get()
                    .get(&context.request.id)
                    .ok_or(ClientError(AdminDisappearFileError::FileNonexistent))?;
                if meta.is_root() {
                    return Err(ClientError(AdminDisappearFileError::RootModificationInvalid));
                }
                meta.owner()
            };
            let mut tree = ServerTree::new(
                owner,
                &mut db.owned_files,
                &mut db.shared_files,
                &mut db.file_children,
                &mut db.metas,
            )?
            .to_lazy();

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
                        }
                    }
                }
            }

            for id in metas_to_delete {
                let meta = db
                    .metas
                    .remove(&id)?
                    .ok_or(ClientError(AdminDisappearFileError::FileNonexistent))?;

                // maintain index: owned_files
                let owner = meta.owner();

                if !db.owned_files.remove(&owner, &id)? {
                    error!(
                        ?id,
                        ?owner,
                        "attempted to disappear a file, owner or id not present in owned_files"
                    );
                }

                // maintain index: shared_files
                for user_access_key in meta.user_access_keys() {
                    let sharee = Owner(user_access_key.encrypted_for);
                    if !db.shared_files.remove(&sharee, &id)? {
                        error!(
                            ?id,
                            ?sharee,
                            "attempted to disappear a file, a sharee didn't have it shared"
                        );
                    }
                }

                // maintain index: file_children
                let parent = *meta.parent();
                if !db.file_children.remove(meta.parent(), &id)? {
                    error!(
                        ?id,
                        ?parent,
                        "attempted to disappear a file, the parent didn't have it as a child"
                    );
                }
            }

            let username = db
                .accounts
                .get()
                .get(&Owner(context.public_key))
                .map(|account| account.username.clone())
                .unwrap_or_else(|| "~unknown~".to_string());
            warn!(?username, ?context.request.id, "Disappeared file");

            tx.drop_safely()?;
        }

        for (id, version) in docs_to_delete {
            self.document_service.delete(&id, &version).await?;
        }

        Ok(())
    }

    pub async fn admin_validate_account(
        &self, context: RequestContext<AdminValidateAccountRequest>,
    ) -> Result<AdminValidateAccount, ServerError<AdminValidateAccountError>> {
        let request = &context.request;
        let mut db = self.index_db.lock().await;
        if !Self::is_admin::<AdminValidateAccountError>(
            &db,
            &context.public_key,
            &self.config.admin.admins,
        )? {
            return Err(ClientError(AdminValidateAccountError::NotPermissioned));
        }

        let owner = *db
            .usernames
            .get()
            .get(&request.username)
            .ok_or(ClientError(AdminValidateAccountError::UserNotFound))?;

        Ok(self.validate_account_helper(&mut db, owner)?)
    }

    pub fn validate_account_helper(
        &self, db: &mut ServerDb, owner: Owner,
    ) -> LbResult<AdminValidateAccount> {
        let mut result = AdminValidateAccount::default();

        let mut tree = ServerTree::new(
            owner,
            &mut db.owned_files,
            &mut db.shared_files,
            &mut db.file_children,
            &mut db.metas,
        )?
        .to_lazy();

        for id in tree.ids() {
            if !tree.calculate_deleted(&id)? {
                let file = tree.find(&id)?;
                if file.is_document() && file.document_hmac().is_some() {
                    if file.file.timestamped_value.value.doc_size().is_none() {
                        result.documents_missing_size.push(id);
                    }

                    if !self
                        .document_service
                        .exists(&id, file.document_hmac().unwrap())
                    {
                        result.documents_missing_content.push(id);
                    }
                }
            }
        }

        let validation_res = tree.stage(None).validate(owner);
        match validation_res {
            Ok(_) => {}
            Err(err) => match err.kind {
                LbErrKind::Validation(validation) => {
                    result.tree_validation_failures.push(validation)
                }
                _ => {
                    error!(?owner, ?err, "Unexpected error while validating tree")
                }
            },
        }

        Ok(result)
    }

    pub async fn admin_validate_server(
        &self, context: RequestContext<AdminValidateServerRequest>,
    ) -> Result<AdminValidateServer, ServerError<AdminValidateServerError>> {
        let mut db = self.index_db.lock().await;
        let db = db.deref_mut();

        if !Self::is_admin::<AdminValidateServerError>(
            db,
            &context.public_key,
            &self.config.admin.admins,
        )? {
            return Err(ClientError(AdminValidateServerError::NotPermissioned));
        }

        let mut result: AdminValidateServer = Default::default();

        let mut deleted_ids = HashSet::new();
        for (id, meta) in db.metas.get().clone() {
            // todo: optimize
            let mut tree = ServerTree::new(
                meta.owner(),
                &mut db.owned_files,
                &mut db.shared_files,
                &mut db.file_children,
                &mut db.metas,
            )?
            .to_lazy();
            if tree.calculate_deleted(&id)? {
                deleted_ids.insert(id);
            }
        }

        // validate accounts
        for (owner, account) in db.accounts.get().clone() {
            let validation = self.validate_account_helper(db, owner)?;
            if !validation.is_empty() {
                result
                    .users_with_validation_failures
                    .insert(account.username, validation);
            }
        }

        // validate index: usernames
        for (username, owner) in db.usernames.get().clone() {
            if let Some(account) = db.accounts.get().get(&owner) {
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
        for (_, account) in db.accounts.get().clone() {
            if db.usernames.get().get(&account.username).is_none() {
                result
                    .usernames_unmapped_to_accounts
                    .insert(account.username.clone());
            }
        }

        // validate index: owned_files
        for (owner, ids) in db.owned_files.get().clone() {
            for id in ids {
                if let Some(meta) = db.metas.get().get(&id) {
                    if meta.owner() != owner {
                        insert(&mut result.owners_mapped_to_unowned_files, owner, id);
                    }
                } else {
                    insert(&mut result.owners_mapped_to_nonexistent_files, owner, id);
                }
            }
        }
        for (id, meta) in db.metas.get().clone() {
            if let Some(ids) = db.owned_files.get().get(&meta.owner()) {
                if !ids.contains(&id) {
                    insert(&mut result.owners_unmapped_to_owned_files, meta.owner(), *meta.id());
                }
            } else {
                result.owners_unmapped.insert(meta.owner());
            }
        }

        // validate index: shared_files
        for (sharee, ids) in db.shared_files.get().clone() {
            for id in ids {
                if let Some(meta) = db.metas.get().get(&id) {
                    if !meta.user_access_keys().iter().any(|k| {
                        !k.deleted && k.encrypted_for == sharee.0 && k.encrypted_by != sharee.0
                    }) {
                        insert(&mut result.sharees_mapped_to_unshared_files, sharee, id);
                    }
                } else {
                    insert(&mut result.sharees_mapped_to_nonexistent_files, sharee, id);
                }
                if deleted_ids.contains(&id) {
                    insert(&mut result.sharees_mapped_for_deleted_files, sharee, id);
                }
            }
        }
        for (id, meta) in db.metas.get().clone() {
            // check for implicit deletion (can't use server tree which depends on index)
            let mut deleted = false;
            let mut ancestor = meta.clone();
            loop {
                if ancestor.explicitly_deleted() {
                    deleted = true;
                    break;
                }
                if ancestor.is_root() {
                    break;
                }
                match db.metas.get().get(ancestor.parent()) {
                    Some(parent) => ancestor = parent.clone(),
                    None => {
                        error!("missing parent for file {:?}", ancestor.parent());
                        deleted = true;
                        break;
                    }
                }
            }
            if deleted {
                continue;
            }

            for k in meta.user_access_keys() {
                if k.deleted {
                    continue;
                }
                let sharee = Owner(k.encrypted_for);
                if let Some(ids) = db.shared_files.get().get(&sharee) {
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
        for (parent_id, child_ids) in db.file_children.get().clone() {
            for child_id in child_ids {
                if let Some(meta) = db.metas.get().get(&child_id) {
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
        for (id, meta) in db.metas.get().clone() {
            if let Some(child_ids) = db.file_children.get().get(meta.parent()) {
                if meta.is_root() && child_ids.contains(&id) {
                    result.files_mapped_as_parent_to_self.insert(id);
                } else if !meta.is_root() && !child_ids.contains(&id) {
                    insert(&mut result.files_unmapped_as_parent_to_children, *meta.parent(), id);
                }
            } else {
                result.files_unmapped_as_parent.insert(*meta.parent());
            }
        }

        // validate presence of documents
        for (id, meta) in db.metas.get().clone() {
            if let Some(hmac) = meta.document_hmac() {
                if !deleted_ids.contains(&id) && !self.document_service.exists(&id, hmac) {
                    result.files_with_hmacs_and_no_contents.insert(id);
                }
            }
        }

        Ok(result)
    }

    pub async fn admin_file_info(
        &self, context: RequestContext<AdminFileInfoRequest>,
    ) -> Result<AdminFileInfoResponse, ServerError<AdminFileInfoError>> {
        let request = &context.request;
        let mut db = self.index_db.lock().await;
        let db = db.deref_mut();
        if !Self::is_admin::<AdminFileInfoError>(
            db,
            &context.public_key,
            &self.config.admin.admins,
        )? {
            return Err(ClientError(AdminFileInfoError::NotPermissioned));
        }

        let file = db
            .metas
            .get()
            .get(&request.id)
            .ok_or(ClientError(AdminFileInfoError::FileNonexistent))?
            .clone();

        let mut tree = ServerTree::new(
            file.owner(),
            &mut db.owned_files,
            &mut db.shared_files,
            &mut db.file_children,
            &mut db.metas,
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
    }

    pub async fn admin_rebuild_index(
        &self, context: RequestContext<AdminRebuildIndexRequest>,
    ) -> Result<(), ServerError<AdminRebuildIndexError>> {
        let mut db = self.index_db.lock().await;

        match context.request.index {
            ServerIndex::OwnedFiles => {
                db.owned_files.clear()?;
                for owner in db.accounts.get().clone().keys() {
                    db.owned_files.create_key(*owner)?;
                }
                for (id, file) in db.metas.get().clone() {
                    db.owned_files.insert(file.owner(), id)?;
                }
            }
            ServerIndex::SharedFiles => {
                db.shared_files.clear()?;
                for owner in db.accounts.get().clone().keys() {
                    db.shared_files.create_key(*owner)?;
                }
                for (id, file) in db.metas.get().clone() {
                    // check for implicit deletion (can't use server tree which depends on index)
                    let mut deleted = false;
                    let mut ancestor = file.clone();
                    loop {
                        if ancestor.explicitly_deleted() {
                            deleted = true;
                            break;
                        }
                        if ancestor.is_root() {
                            break;
                        }
                        match db.metas.get().get(ancestor.parent()) {
                            Some(parent) => ancestor = parent.clone(),
                            None => {
                                error!("missing parent for file {:?}", ancestor.parent());
                                deleted = true;
                                break;
                            }
                        }
                    }

                    if !deleted {
                        for user_access_key in file.user_access_keys() {
                            if !user_access_key.deleted
                                && user_access_key.encrypted_for != user_access_key.encrypted_by
                            {
                                db.shared_files
                                    .insert(Owner(user_access_key.encrypted_for), id)?;
                            }
                        }
                    }
                }
            }
            ServerIndex::FileChildren => {
                db.file_children.clear()?;
                for id in db.metas.get().clone().keys() {
                    db.file_children.create_key(*id)?;
                }
                for (id, file) in db.metas.get().clone() {
                    db.file_children.insert(*file.parent(), id)?;
                }
            }
        }
        Ok(())
    }
}

fn insert<K: Hash + Eq, V: Hash + Eq>(map: &mut HashMap<K, HashSet<V>>, k: K, v: V) {
    map.entry(k).or_default().insert(v);
}
