use crate::model::errors::core_err_unexpected;
use crate::model::repo::RepoSource;
use crate::model::repo::RepoState;
use crate::pure_functions::files;
use crate::repo::document_repo;
use crate::repo::schema::OneKey;
use crate::service::api_service;
use crate::service::file_compression_service;
use crate::service::file_encryption_service;
use crate::CoreError::RootNonexistent;
use crate::{Config, CoreError, RequestContext};
use itertools::Itertools;
use lockbook_crypto::pubkey;
use lockbook_models::api::GetPublicKeyRequest;
use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::crypto::EncryptedDocument;
use lockbook_models::crypto::UserAccessInfo;
use lockbook_models::crypto::UserAccessMode;
use lockbook_models::file_metadata::FileMetadataDiff;
use lockbook_models::file_metadata::FileType;
use lockbook_models::file_metadata::Owner;
use lockbook_models::file_metadata::ShareMode;
use lockbook_models::file_metadata::{DecryptedFileMetadata, DecryptedFiles};
use lockbook_models::file_metadata::{EncryptedFileMetadata, EncryptedFiles};
use lockbook_models::tree::FileMetaVecExt;
use lockbook_models::tree::TreeError;
use lockbook_models::tree::{FileMetaMapExt, FileMetadata};
use lockbook_models::utils;
use sha2::Digest;
use sha2::Sha256;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

impl RequestContext<'_, '_> {
    pub fn create_file(
        &mut self, config: &Config, name: &str, parent: Uuid, file_type: FileType,
    ) -> Result<DecryptedFileMetadata, CoreError> {
        let account = self.get_account()?;
        self.get_not_deleted_metadata(RepoSource::Local, parent)?;
        let all_metadata = self.get_all_metadata(RepoSource::Local)?;
        let metadata = files::apply_create(
            &Owner(account.public_key()),
            &all_metadata,
            file_type,
            parent,
            name,
        )?;
        self.insert_metadatum(config, RepoSource::Local, &metadata)?;
        Ok(metadata)
    }

    pub fn root_id(&self) -> Result<Uuid, CoreError> {
        self.tx.root.get(&OneKey).ok_or(RootNonexistent)
    }

    pub fn root(&mut self) -> Result<DecryptedFileMetadata, CoreError> {
        let root_id = self.root_id()?;
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        files.maybe_find(root_id).ok_or(RootNonexistent)
    }

    /// Adds or updates the metadata of a file on disk.
    pub fn insert_metadatum(
        &mut self, config: &Config, source: RepoSource, metadata: &DecryptedFileMetadata,
    ) -> Result<(), CoreError> {
        self.insert_metadata(config, source, &HashMap::from([(metadata.id, metadata.clone())]))
    }

    pub fn insert_metadata(
        &mut self, config: &Config, source: RepoSource, metadata_changes: &DecryptedFiles,
    ) -> Result<(), CoreError> {
        let all_metadata = self.get_all_metadata(source)?;
        self.insert_metadata_given_decrypted_metadata(source, &all_metadata, metadata_changes)?;
        if source == RepoSource::Local {
            self.insert_new_docs(config, &all_metadata, metadata_changes)?;
        }
        Ok(())
    }

    pub fn get_metadata(
        &mut self, source: RepoSource, id: Uuid,
    ) -> Result<DecryptedFileMetadata, CoreError> {
        self.maybe_get_metadata(source, id)
            .and_then(|f| f.ok_or(CoreError::FileNonexistent))
    }

    pub fn get_all_not_deleted_metadata(
        &mut self, source: RepoSource,
    ) -> Result<DecryptedFiles, CoreError> {
        Ok(self.get_all_metadata(source)?.filter_not_deleted()?)
    }

    // TODO: should this even exist? Could impl get on a tx with a source and it will do the lookup
    //       at that point in time
    pub fn get_all_metadata(&mut self, source: RepoSource) -> Result<DecryptedFiles, CoreError> {
        let account = self.get_account()?;
        let base: EncryptedFiles = self.tx.base_metadata.get_all();
        match source {
            RepoSource::Base => file_encryption_service::decrypt_metadata(
                &account,
                &base,
                &mut self.data_cache.key_cache,
            ),
            RepoSource::Local => {
                let local: EncryptedFiles = self.tx.local_metadata.get_all();
                let staged = base.stage(local);
                file_encryption_service::decrypt_metadata(
                    &account,
                    &staged,
                    &mut self.data_cache.key_cache,
                )
            }
        }
    }

    pub fn get_pending_shares(
        &mut self, source: RepoSource,
    ) -> Result<Vec<DecryptedFileMetadata>, CoreError> {
        // pending shares = metadata shared with user for which no link exists
        let username = self.get_account()?.username;
        let all_metadata = self.get_all_metadata(source)?;
        let all_metadata = all_metadata.filter_not_deleted()?;
        let shared_metadata = all_metadata.iter().map(|(_, f)| f).filter(|f| {
            f.shares
                .iter()
                .any(|s| s.encrypted_for_username == username && s.mode != UserAccessMode::Owner)
        });
        let pending_shares = shared_metadata
            .into_iter()
            .filter(|f| {
                all_metadata.iter().map(|(_, f)| f).all(|f2| !{
                    if let FileType::Link { linked_file } = f2.file_type {
                        linked_file == f.id
                    } else {
                        false
                    }
                })
            })
            .cloned()
            .collect();
        Ok(pending_shares)
    }

    /// Adds or updates the content of a document on disk.
    /// Disk optimization opportunity: this function needlessly writes to disk when setting local content = base content.
    /// CPU optimization opportunity: this function needlessly decrypts all metadata rather than just ancestors of metadata parameter.
    pub fn insert_document(
        &mut self, config: &Config, source: RepoSource, metadata: &DecryptedFileMetadata,
        document: &[u8],
    ) -> Result<(), CoreError> {
        // check that document exists and is a document
        self.get_metadata(RepoSource::Local, metadata.id)?;
        if metadata.is_folder() {
            return Err(CoreError::FileNotDocument);
        }

        // encrypt document and compute digest
        let digest = Sha256::digest(document);
        let compressed_document = file_compression_service::compress(document)?;
        let encrypted_document =
            file_encryption_service::encrypt_document(&compressed_document, metadata)?;

        // perform insertions
        document_repo::insert(config, source, metadata.id, &encrypted_document)?;
        match source {
            RepoSource::Local => {
                self.tx.local_digest.insert(metadata.id, digest.to_vec());
            }
            RepoSource::Base => {
                self.tx.base_digest.insert(metadata.id, digest.to_vec());
            }
        }

        let opposite_digest = match source.opposite() {
            RepoSource::Local => self.tx.local_digest.get(&metadata.id),
            RepoSource::Base => self.tx.base_digest.get(&metadata.id),
        };

        // remove local if local == base
        if let Some(opposite) = opposite_digest {
            if utils::slices_equal(&opposite, &digest) {
                self.tx.local_digest.delete(metadata.id);
                document_repo::delete(config, RepoSource::Local, metadata.id)?;
            }
        }

        Ok(())
    }

    /// Adds or updates the metadata of files on disk.
    /// Disk optimization opportunity: this function needlessly writes to disk when setting local metadata = base metadata.
    /// CPU optimization opportunity: this function needlessly decrypts all metadata rather than just ancestors of metadata parameter.
    fn insert_metadata_given_decrypted_metadata(
        &mut self, source: RepoSource, all_metadata: &DecryptedFiles,
        metadata_changes: &DecryptedFiles,
    ) -> Result<(), CoreError> {
        // encrypt metadata
        let all_metadata_with_changes_staged = all_metadata
            .stage_with_source(metadata_changes)
            .into_iter()
            .map(|(id, (f, _))| (id, f))
            .collect::<DecryptedFiles>();

        let changes = metadata_changes.clone();
        let mut parents = HashMap::new();

        for (_, f) in changes.iter() {
            let ancestors = files::find_ancestors(&all_metadata_with_changes_staged, f.parent);
            parents.extend(ancestors);
        }

        let changes_and_parents = changes.into_iter().chain(parents.into_iter()).collect();
        let necessary_metadata_encrypted =
            file_encryption_service::encrypt_metadata(&self.get_account()?, &changes_and_parents)?;

        for (&id, metadatum) in metadata_changes {
            let encrypted_metadata = necessary_metadata_encrypted.find(id)?;

            // perform insertion
            match source {
                RepoSource::Local => {
                    self.tx
                        .local_metadata
                        .insert(encrypted_metadata.id, encrypted_metadata.clone());
                }
                RepoSource::Base => {
                    self.tx
                        .base_metadata
                        .insert(encrypted_metadata.id, encrypted_metadata.clone());
                }
            }

            // remove local if local == base
            let opposite_metadata = match source.opposite() {
                RepoSource::Local => self.tx.local_metadata.get(&encrypted_metadata.id),
                RepoSource::Base => self.tx.base_metadata.get(&encrypted_metadata.id),
            };

            if let Some(opposite) = opposite_metadata {
                if utils::slices_equal(&opposite.name.hmac, &encrypted_metadata.name.hmac)
                    && opposite.parent == metadatum.parent
                    && opposite.deleted == metadatum.deleted
                    && opposite.user_access_keys == metadatum.shares
                {
                    self.tx.local_metadata.delete(id);
                }
            }

            // update root
            if metadatum.parent == id {
                self.tx.root.insert(OneKey {}, id);
            }
        }

        Ok(())
    }

    fn insert_new_docs(
        &mut self, config: &Config, local_metadata: &DecryptedFiles,
        metadata_changes: &DecryptedFiles,
    ) -> Result<(), CoreError> {
        for metadatum in metadata_changes.values() {
            if metadatum.is_document() && local_metadata.maybe_find(metadatum.id).is_none() {
                self.insert_document(config, RepoSource::Local, metadatum, &[])?;
            }
        }
        Ok(())
    }

    pub fn maybe_get_metadata(
        &mut self, source: RepoSource, id: Uuid,
    ) -> Result<Option<DecryptedFileMetadata>, CoreError> {
        let all_metadata = self.get_all_metadata(source)?;
        Ok(all_metadata.maybe_find(id))
    }

    pub fn get_not_deleted_metadata(
        &mut self, source: RepoSource, id: Uuid,
    ) -> Result<DecryptedFileMetadata, CoreError> {
        self.maybe_get_not_deleted_metadata(source, id)
            .and_then(|f| f.ok_or(CoreError::FileNonexistent))
    }

    pub fn maybe_get_not_deleted_metadata(
        &mut self, source: RepoSource, id: Uuid,
    ) -> Result<Option<DecryptedFileMetadata>, CoreError> {
        let all_not_deleted_metadata = self.get_all_not_deleted_metadata(source)?;
        Ok(all_not_deleted_metadata.maybe_find(id))
    }

    pub fn get_children(&mut self, id: Uuid) -> Result<DecryptedFiles, CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        // todo(sharing): this (and other uses) mean we do not support links pointing to links, so make sure you prevent that on link create/update
        let children = files
            .find_children(id)
            .iter()
            .map(|(_, f)| {
                if let FileType::Link { linked_file } = f.file_type {
                    files.find(linked_file).map(|mut f2| {
                        // return the linked file, but use the link's name
                        f2.decrypted_name = f.decrypted_name.clone();
                        f2
                    })
                } else {
                    Ok(f.clone())
                }
            })
            .collect::<Result<Vec<DecryptedFileMetadata>, TreeError>>();
        Ok(children?.to_map())
    }

    pub fn get_and_get_children_recursively(
        &mut self, id: Uuid,
    ) -> Result<DecryptedFiles, CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        let file_and_descendants = files::find_with_descendants(&files, id)?
            .iter()
            .map(|(_, f)| {
                if let FileType::Link { linked_file } = f.file_type {
                    files.find(linked_file)
                } else {
                    Ok(f.clone())
                }
            })
            .collect::<Result<Vec<DecryptedFileMetadata>, TreeError>>();
        Ok(file_and_descendants?.to_map())
    }

    pub fn delete_file(&mut self, config: &Config, id: Uuid) -> Result<(), CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        let file = files::apply_delete(&Owner(self.get_public_key()?), &files, id)?;
        self.insert_metadatum(config, RepoSource::Local, &file)?;
        self.prune_deleted(config)
    }

    /// Removes deleted files which are safe to delete. Call this function after a set of operations rather than in-between
    /// each operation because otherwise you'll prune e.g. a file that was moved out of a folder that was deleted.
    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn prune_deleted(&mut self, config: &Config) -> Result<(), CoreError> {
        // If a file is deleted or has a deleted ancestor, we say that it is deleted. Whether a file is deleted is specific
        // to the source (base or local). We cannot prune (delete from disk) a file in one source and not in the other in
        // order to preserve the semantics of having a file present on one, the other, or both (unmodified/new/modified).
        // For a file to be pruned, it must be deleted on both sources but also have no non-deleted descendants on either
        // source - otherwise, the metadata for those descendants can no longer be decrypted. For an example of a situation
        // where this is important, see the test prune_deleted_document_moved_from_deleted_folder_local_only.

        // find files deleted on base and local; new deleted local files are also eligible
        let all_base_metadata = self.get_all_metadata(RepoSource::Base)?;
        let deleted_base_metadata = all_base_metadata.deleted_status()?.deleted;
        let all_local_metadata = self.get_all_metadata(RepoSource::Local)?;
        let deleted_local_metadata = all_local_metadata.deleted_status()?.deleted;
        let deleted_both_metadata = deleted_base_metadata
            .into_iter()
            .filter(|id| deleted_local_metadata.contains(id));
        let prune_eligible_ids =
            deleted_local_metadata
                .iter()
                .filter_map(|id| {
                    if all_base_metadata.maybe_find(*id).is_none() {
                        Some(*id)
                    } else {
                        None
                    }
                })
                .chain(deleted_both_metadata)
                .collect::<HashSet<Uuid>>();

        // exclude files with not deleted descendants i.e. exclude files that are the ancestors of not deleted files
        let all_ids = all_base_metadata
            .keys()
            .chain(all_local_metadata.keys())
            .cloned()
            .collect::<HashSet<Uuid>>();
        let not_deleted_either_ids = all_ids
            .into_iter()
            .filter(|id| !prune_eligible_ids.contains(id))
            .collect::<HashSet<Uuid>>();
        let ancestors_of_not_deleted_base_ids = not_deleted_either_ids
            .iter()
            .flat_map(|&id| files::find_ancestors(&all_base_metadata, id).into_keys())
            .collect::<HashSet<Uuid>>();
        let ancestors_of_not_deleted_local_ids = not_deleted_either_ids
            .iter()
            .flat_map(|&id| files::find_ancestors(&all_local_metadata, id).into_keys())
            .collect::<HashSet<Uuid>>();
        let deleted_both_without_deleted_descendants_ids =
            prune_eligible_ids.into_iter().filter(|id| {
                !ancestors_of_not_deleted_base_ids.contains(id)
                    && !ancestors_of_not_deleted_local_ids.contains(id)
            });

        // remove files from disk
        for id in deleted_both_without_deleted_descendants_ids {
            self.delete_metadata(id);
            if all_local_metadata.find_ref(id)?.is_document() {
                self.delete_document(config, id)?;
            }
        }
        Ok(())
    }

    fn delete_metadata(&mut self, id: Uuid) {
        self.tx.local_metadata.delete(id);
        self.tx.base_metadata.delete(id);
    }

    fn delete_document(&mut self, config: &Config, id: Uuid) -> Result<(), CoreError> {
        document_repo::delete(config, RepoSource::Local, id)?;
        document_repo::delete(config, RepoSource::Base, id)?;
        self.tx.local_digest.delete(id);
        self.tx.base_digest.delete(id);

        Ok(())
    }

    pub fn read_document_old(
        &mut self, config: &Config, id: Uuid,
    ) -> Result<DecryptedDocument, CoreError> {
        self.read_document(config, RepoSource::Local, id)
    }

    pub fn read_document(
        &mut self, config: &Config, source: RepoSource, id: Uuid,
    ) -> Result<DecryptedDocument, CoreError> {
        let metas = self.get_all_metadata(source)?.filter_not_deleted()?;

        let meta = metas.get(&id).ok_or(CoreError::FileNonexistent)?;

        maybe_get_document(config, source, meta)?.ok_or(CoreError::FileNonexistent)
    }

    pub fn maybe_get_not_deleted_document(
        &self, config: &Config, source: RepoSource, metadata: &DecryptedFiles, id: Uuid,
    ) -> Result<Option<DecryptedDocument>, CoreError> {
        if let Some(metadata) = metadata.maybe_find(id) {
            maybe_get_document(config, source, &metadata)
        } else {
            Ok(None)
        }
    }

    pub fn save_document_to_disk(
        &mut self, config: &Config, id: Uuid, location: &str,
    ) -> Result<(), CoreError> {
        let document = self.read_document(config, RepoSource::Local, id)?;
        files::save_document_to_disk(&document, location.to_string())
    }

    pub fn rename_file(
        &mut self, config: &Config, id: Uuid, new_name: &str,
    ) -> Result<(), CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        let file = files::apply_rename(&Owner(self.get_public_key()?), &files, id, new_name)?;
        self.insert_metadatum(config, RepoSource::Local, &file)
    }

    pub fn move_file(
        &mut self, config: &Config, id: Uuid, new_parent: Uuid,
    ) -> Result<(), CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        let file = files::apply_move(&Owner(self.get_public_key()?), &files, id, new_parent)?;
        self.insert_metadatum(config, RepoSource::Local, &file)
    }

    pub fn share_file(
        &mut self, config: &Config, id: Uuid, username: &str, mode: ShareMode,
    ) -> Result<(), CoreError> {
        let user = Owner(self.get_public_key()?);
        let access_mode = match mode {
            ShareMode::Write => UserAccessMode::Write,
            ShareMode::Read => UserAccessMode::Read,
        };

        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        let files = files.filter_not_deleted()?;
        let mut file = files.find(id)?;

        files::validate_not_root(&file)?;
        if mode == ShareMode::Write && file.owner.0 != self.get_public_key()? {
            return Err(CoreError::InsufficientPermission);
        }

        let account = self.get_account()?;
        let public_key = api_service::request(
            &account,
            GetPublicKeyRequest { username: String::from(username) },
        )
        .map_err(CoreError::from)?
        .key;
        let access_key = file_encryption_service::encrypt_user_access_key(
            &file.decrypted_access_key,
            &account.private_key,
            &public_key,
        )?;

        // check for and remove duplicate shares
        if let Some(existing_access_mode) = file.get_access_mode(&Owner(public_key)) {
            if existing_access_mode == access_mode {
                return Err(CoreError::ShareAlreadyExists);
            } else {
                file.shares = file
                    .shares
                    .into_iter()
                    .filter(|s| s.encrypted_for_public_key != public_key)
                    .collect();
            }
        }

        let share_key =
            pubkey::get_aes_key(&account.private_key, &public_key).map_err(core_err_unexpected)?;
        file.shares.push(UserAccessInfo {
            mode: access_mode,
            encrypted_by_username: account.username.clone(),
            encrypted_by_public_key: account.public_key(),
            encrypted_for_username: String::from(username),
            encrypted_for_public_key: public_key,
            access_key,
            file_name: file_encryption_service::encrypt_file_name(
                &file.decrypted_name,
                &share_key,
            )?,
        });

        let staged_changes = HashMap::with(file.clone());
        if !files.get_shared_links(&user, &staged_changes)?.is_empty() {
            return Err(CoreError::LinkInSharedFolder);
        }

        self.insert_metadatum(config, RepoSource::Local, &file)?;
        Ok(())
    }

    pub fn delete_pending_share(&mut self, config: &Config, id: Uuid) -> Result<(), CoreError> {
        let username = self.get_account()?.username;
        let mut file = self.get_metadata(RepoSource::Local, id)?;

        // todo(sharing): make sure we don't remove shares for files we own (and probably a few other checks)
        file.shares = file
            .shares
            .into_iter()
            .filter(|s| s.encrypted_for_username == username)
            .collect();

        self.insert_metadatum(config, RepoSource::Local, &file)?;
        Ok(())
    }

    pub fn get_all_with_document_changes(
        &mut self, config: &Config,
    ) -> Result<Vec<Uuid>, CoreError> {
        let all = self.get_all_metadata(RepoSource::Local)?;
        let not_deleted = all.filter_not_deleted()?;
        let not_deleted_with_document_changes = not_deleted
            .documents()
            .iter()
            .map(|id| {
                document_repo::maybe_get(config, RepoSource::Local, *id).map(|r| r.map(|_| *id))
            })
            .collect::<Result<Vec<Option<Uuid>>, CoreError>>()?
            .into_iter()
            .flatten()
            .collect();
        Ok(not_deleted_with_document_changes)
    }

    pub fn get_all_metadata_with_encrypted_changes(
        &mut self, source: RepoSource, changes: &EncryptedFiles,
    ) -> Result<(DecryptedFiles, EncryptedFiles), CoreError> {
        let account = self.get_account()?;
        let base = self.tx.base_metadata.get_all();
        let sourced = match source {
            RepoSource::Local => {
                let local = self.tx.local_metadata.get_all();
                base.stage_with_source(&local)
                    .into_iter()
                    .map(|(id, (f, _))| (id, f))
                    .collect()
            }
            RepoSource::Base => base,
        };

        let staged = sourced
            .stage_with_source(changes)
            .into_iter()
            .map(|(id, (f, _))| (id, f))
            .collect::<EncryptedFiles>();

        let root = match self.tx.root.get(&OneKey) {
            Some(id) => staged.find(id),
            None => staged.find_root(),
        }?;
        let non_orphans = files::find_with_descendants(&staged, root.id)?
            .into_iter()
            .map(|(_, f)| f)
            .chain(
                staged
                    .iter()
                    .map(|(_, f)| f)
                    .filter(|f| {
                        f.user_access_keys
                            .iter()
                            .any(|k| k.encrypted_for_username == account.username)
                    })
                    .cloned(),
            )
            .collect::<Vec<EncryptedFileMetadata>>()
            .to_map();
        let mut staged_non_orphans = HashMap::new();
        let mut encrypted_orphans = HashMap::new();
        for (_, f) in staged {
            if non_orphans.maybe_find(f.id).is_some() {
                // only decrypt non-orphans
                staged_non_orphans.push(f);
            } else {
                // deleted orphaned files
                encrypted_orphans.push(f);
            }
        }

        Ok((
            file_encryption_service::decrypt_metadata(
                &account,
                &staged_non_orphans,
                &mut self.data_cache.key_cache,
            )?,
            encrypted_orphans,
        ))
    }

    pub fn get_all_metadata_state(
        &mut self,
    ) -> Result<Vec<RepoState<DecryptedFileMetadata>>, CoreError> {
        let account = self.get_account()?;
        let base_encrypted = self.tx.base_metadata.get_all();
        let base = file_encryption_service::decrypt_metadata(
            &account,
            &base_encrypted,
            &mut self.data_cache.key_cache,
        )?;
        let local = {
            let local_encrypted = self.tx.local_metadata.get_all();
            let staged = base_encrypted
                .stage_with_source(&local_encrypted)
                .into_iter()
                .map(|(id, (f, _))| (id, f))
                .collect::<EncryptedFiles>();
            let decrypted = file_encryption_service::decrypt_metadata(
                &account,
                &staged,
                &mut self.data_cache.key_cache,
            )?;
            decrypted
                .into_iter()
                .filter(|(d_id, _)| local_encrypted.keys().any(|l_id| l_id == d_id))
                .collect::<DecryptedFiles>()
        };

        let new = local
            .values()
            .filter(|l| !base.values().any(|b| l.id == b.id))
            .map(|l| RepoState::New(l.clone()));
        let unmodified = base
            .values()
            .filter(|b| !local.values().any(|l| l.id == b.id))
            .map(|b| RepoState::Unmodified(b.clone()));
        let modified = base.values().filter_map(|b| {
            local
                .maybe_find(b.id)
                .map(|l| RepoState::Modified { base: b.clone(), local: l })
        });

        Ok(new.chain(unmodified).chain(modified).collect())
    }

    /// Updates base metadata to match local metadata.
    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn promote_metadata(&mut self) -> Result<(), CoreError> {
        let base_metadata = self.tx.base_metadata.get_all();
        let local_metadata = self.tx.local_metadata.get_all();
        let staged_metadata = base_metadata.stage_with_source(&local_metadata);

        self.tx.base_metadata.clear();

        for (metadata, _) in staged_metadata.values() {
            self.tx.base_metadata.insert(metadata.id, metadata.clone());
        }

        self.tx.local_metadata.clear();

        Ok(())
    }

    pub fn get_all_metadata_changes(&self) -> Result<Vec<FileMetadataDiff>, CoreError> {
        let local = self.tx.local_metadata.get_all().into_values().collect_vec();
        let base = self.tx.base_metadata.get_all().into_values().collect_vec();

        let new = local
            .iter()
            .filter(|l| !base.iter().any(|r| r.id == l.id))
            .map(FileMetadataDiff::new);
        let changed = local
            .iter()
            .filter_map(|l| base.iter().find(|r| r.id == l.id).map(|r| (l, r)))
            .map(|(l, r)| FileMetadataDiff::new_diff(r.parent, &r.name, l));

        Ok(new.chain(changed).collect())
    }

    /// Updates base documents to match local documents.
    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn promote_documents(&mut self, config: &Config) -> Result<(), CoreError> {
        let base_metadata = self.tx.base_metadata.get_all();
        let local_metadata = self.tx.local_metadata.get_all();
        let staged_metadata = base_metadata.stage_with_source(&local_metadata);
        let staged_everything = staged_metadata
            .values()
            .map(|(f, _)| {
                Ok((
                    f.clone(),
                    match document_repo::maybe_get(config, RepoSource::Local, f.id)? {
                        Some(document) => Some(document),
                        None => document_repo::maybe_get(config, RepoSource::Base, f.id)?,
                    },
                    match self.tx.local_digest.get(&f.id) {
                        Some(digest) => Some(digest),
                        None => self.tx.base_digest.get(&f.id),
                    },
                ))
            })
            .collect::<Result<
                Vec<(EncryptedFileMetadata, Option<EncryptedDocument>, Option<Vec<u8>>)>,
                CoreError,
            >>()?;

        document_repo::delete_all(config, RepoSource::Base)?;
        self.tx.base_digest.clear();

        for (metadata, maybe_document, maybe_digest) in staged_everything {
            if let Some(document) = maybe_document {
                document_repo::insert(config, RepoSource::Base, metadata.id, &document)?;
            }
            if let Some(digest) = maybe_digest {
                self.tx.base_digest.insert(metadata.id, digest);
            }
        }

        document_repo::delete_all(config, RepoSource::Local)?;
        self.tx.local_digest.clear();

        Ok(())
    }

    pub fn get_local_changes(&mut self, config: &Config) -> Result<Vec<Uuid>, CoreError> {
        Ok(self
            .get_all_metadata_changes()?
            .into_iter()
            .map(|f| f.id)
            .chain(self.get_all_with_document_changes(config)?.into_iter())
            .unique()
            .collect())
    }

    pub fn insert_metadata_both_repos(
        &mut self, config: &Config, base_metadata_changes: &DecryptedFiles,
        local_metadata_changes: &DecryptedFiles,
    ) -> Result<(), CoreError> {
        let base_metadata = self.get_all_metadata(RepoSource::Base)?;
        let local_metadata = self.get_all_metadata(RepoSource::Local)?;
        self.insert_metadata_given_decrypted_metadata(
            RepoSource::Base,
            &base_metadata,
            base_metadata_changes,
        )?;
        self.insert_metadata_given_decrypted_metadata(
            RepoSource::Local,
            &local_metadata,
            local_metadata_changes,
        )?;
        self.insert_new_docs(config, &local_metadata, local_metadata_changes)
    }

    pub fn get_metadata_state(
        &mut self, id: Uuid,
    ) -> Result<RepoState<DecryptedFileMetadata>, CoreError> {
        self.maybe_get_metadata_state(id)
            .and_then(|f| f.ok_or(CoreError::FileNonexistent))
    }

    pub fn maybe_get_metadata_state(
        &mut self, id: Uuid,
    ) -> Result<Option<RepoState<DecryptedFileMetadata>>, CoreError> {
        let all_metadata = self.get_all_metadata_state()?;
        Ok(files::maybe_find_state(&all_metadata, id))
    }

    pub fn get_all_document_state(
        &mut self, config: &Config,
    ) -> Result<Vec<RepoState<DecryptedDocument>>, CoreError> {
        let doc_metadata: Vec<RepoState<DecryptedFileMetadata>> = self
            .get_all_metadata_state()?
            .into_iter()
            .filter(|r| r.clone().local().is_document())
            .collect();
        let mut result = Vec::new();
        for doc_metadatum in doc_metadata {
            if let Some(doc_state) = maybe_get_document_state(config, &doc_metadatum)? {
                result.push(doc_state);
            }
        }
        Ok(result)
    }
}

pub fn get_not_deleted_document(
    config: &Config, source: RepoSource, metadata: &DecryptedFiles, id: Uuid,
) -> Result<DecryptedDocument, CoreError> {
    maybe_get_not_deleted_document(config, source, metadata, id)
        .and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn maybe_get_not_deleted_document(
    config: &Config, source: RepoSource, metadata: &DecryptedFiles, id: Uuid,
) -> Result<Option<DecryptedDocument>, CoreError> {
    let maybe_doc_metadata = metadata
        .deleted_status()?
        .not_deleted
        .get(&id)
        .and_then(|id| metadata.get(id));

    if let Some(metadata) = maybe_doc_metadata {
        maybe_get_document(config, source, metadata)
    } else {
        Ok(None)
    }
}

pub fn get_document(
    config: &Config, source: RepoSource, metadata: &DecryptedFileMetadata,
) -> Result<DecryptedDocument, CoreError> {
    maybe_get_document(config, source, metadata).and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn maybe_get_document(
    config: &Config, source: RepoSource, metadata: &DecryptedFileMetadata,
) -> Result<Option<DecryptedDocument>, CoreError> {
    if metadata.file_type != FileType::Document {
        return Err(CoreError::FileNotDocument);
    }
    let maybe_encrypted_document = match source {
        RepoSource::Local => {
            match document_repo::maybe_get(config, RepoSource::Local, metadata.id)? {
                Some(local) => Some(local),
                None => document_repo::maybe_get(config, RepoSource::Base, metadata.id)?,
            }
        }
        RepoSource::Base => document_repo::maybe_get(config, RepoSource::Base, metadata.id)?,
    };

    Ok(match maybe_encrypted_document {
        None => None,
        Some(encrypted_document) => {
            let compressed_document =
                file_encryption_service::decrypt_document(&encrypted_document, metadata)?;
            let document = file_compression_service::decompress(&compressed_document)?;
            Some(document)
        }
    })
}

pub fn maybe_get_document_state(
    config: &Config, metadata: &RepoState<DecryptedFileMetadata>,
) -> Result<Option<RepoState<DecryptedDocument>>, CoreError> {
    if metadata.clone().local().file_type != FileType::Document {
        return Err(CoreError::FileNotDocument);
    }
    let id = metadata.clone().local().id;

    let base = if let Some(base_metadata) = metadata.clone().base() {
        match document_repo::maybe_get(config, RepoSource::Base, id)? {
            None => None,
            Some(encrypted_document) => {
                let compressed_document =
                    file_encryption_service::decrypt_document(&encrypted_document, &base_metadata)?;
                let document = file_compression_service::decompress(&compressed_document)?;
                Some(document)
            }
        }
    } else {
        None
    };
    let local = match document_repo::maybe_get(config, RepoSource::Local, id)? {
        None => None,
        Some(encrypted_document) => {
            let compressed_document = file_encryption_service::decrypt_document(
                &encrypted_document,
                &metadata.clone().local(),
            )?;
            let document = file_compression_service::decompress(&compressed_document)?;
            Some(document)
        }
    };
    Ok(RepoState::from_local_and_base(local, base))
}
