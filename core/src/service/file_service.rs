use std::collections::HashSet;

use hmdb::transaction::Transaction;
use itertools::Itertools;
use sha2::Digest;
use sha2::Sha256;
use uuid::Uuid;

use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::crypto::EncryptedDocument;
use lockbook_models::file_metadata::DecryptedFileMetadata;
use lockbook_models::file_metadata::EncryptedFileMetadata;
use lockbook_models::file_metadata::FileMetadataDiff;
use lockbook_models::file_metadata::FileType;
use lockbook_models::tree::FileMetaExt;
use lockbook_models::utils;

use crate::model::errors::{GetRootError, RenameFileError, SaveDocumentToDiskError};
use crate::model::repo::RepoSource;
use crate::model::repo::RepoState;
use crate::pure_functions::files;
use crate::pure_functions::files::maybe_find_state;
use crate::repo::document_repo;
use crate::repo::schema::{OneKey, Tx};
use crate::service::file_encryption_service;
use crate::service::{file_compression_service, file_service};
use crate::CoreError::RootNonexistent;
use crate::{
    Config, CoreError, CreateFileError, Error, FileDeleteError, GetAndGetChildrenError,
    GetFileByIdError, LbCore, MoveFileError, ReadDocumentError, UnexpectedError,
    WriteToDocumentError,
};

impl Tx<'_> {
    pub fn create_file(
        &mut self, config: &Config, name: &str, parent: Uuid, file_type: FileType,
    ) -> Result<DecryptedFileMetadata, CoreError> {
        let account = self.get_account()?;
        self.get_not_deleted_metadata(RepoSource::Local, parent)?;
        let all_metadata = self.get_all_metadata(RepoSource::Local)?;
        let metadata =
            files::apply_create(&all_metadata, file_type, parent, name, &account.public_key())?;
        self.insert_metadatum(&config, RepoSource::Local, &metadata)?;
        Ok(metadata)
    }
    pub fn root(&self) -> Result<DecryptedFileMetadata, CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;

        match files.maybe_find_root() {
            None => Err(RootNonexistent),
            Some(file_metadata) => Ok(file_metadata),
        }
    }

    /// Adds or updates the metadata of a file on disk.
    pub fn insert_metadatum(
        &mut self, config: &Config, source: RepoSource, metadata: &DecryptedFileMetadata,
    ) -> Result<(), CoreError> {
        self.insert_metadata(config, source, &[metadata.clone()])
    }

    pub fn insert_metadata(
        &mut self, config: &Config, source: RepoSource, metadata_changes: &[DecryptedFileMetadata],
    ) -> Result<(), CoreError> {
        let all_metadata = self.get_all_metadata(source)?;
        self.insert_metadata_given_decrypted_metadata(
            config,
            source,
            &all_metadata,
            metadata_changes,
        )
    }

    pub fn get_metadata(
        &self, source: RepoSource, id: Uuid,
    ) -> Result<DecryptedFileMetadata, CoreError> {
        self.maybe_get_metadata(source, id)
            .and_then(|f| f.ok_or(CoreError::FileNonexistent))
    }

    pub fn get_all_not_deleted_metadata(
        &self, source: RepoSource,
    ) -> Result<Vec<DecryptedFileMetadata>, CoreError> {
        Ok(self.get_all_metadata(source)?.filter_not_deleted()?)
    }

    // TODO: should this even exist? Could impl get on a tx with a source and it will do the lookup
    //       at that point in time
    pub fn get_all_metadata(
        &self, source: RepoSource,
    ) -> Result<Vec<DecryptedFileMetadata>, CoreError> {
        let account = self.get_account()?;
        let base: Vec<EncryptedFileMetadata> = self.base_metadata.get_all().into_values().collect();
        match source {
            RepoSource::Base => file_encryption_service::decrypt_metadata(&account, &base),
            RepoSource::Local => {
                let local: Vec<EncryptedFileMetadata> =
                    self.local_metadata.get_all().into_values().collect();
                let staged = base
                    .stage(&local)
                    .into_iter()
                    .map(|(f, _)| f)
                    .collect::<Vec<EncryptedFileMetadata>>();
                file_encryption_service::decrypt_metadata(&account, &staged)
            }
        }
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
        if metadata.file_type == FileType::Folder {
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
                self.local_digest.insert(metadata.id, digest.to_vec());
            }
            RepoSource::Base => {
                self.base_digest.insert(metadata.id, digest.to_vec());
            }
        }

        let opposite_digest = match source.opposite() {
            RepoSource::Local => self.local_digest.get(&metadata.id),
            RepoSource::Base => self.base_digest.get(&metadata.id),
        };

        // remove local if local == base
        if let Some(opposite) = opposite_digest {
            if utils::slices_equal(&opposite, &digest) {
                self.local_digest.delete(metadata.id);
                document_repo::delete(config, RepoSource::Local, metadata.id)?;
            }
        }

        Ok(())
    }

    /// Adds or updates the metadata of files on disk.
    /// Disk optimization opportunity: this function needlessly writes to disk when setting local metadata = base metadata.
    /// CPU optimization opportunity: this function needlessly decrypts all metadata rather than just ancestors of metadata parameter.
    fn insert_metadata_given_decrypted_metadata(
        &mut self, config: &Config, source: RepoSource, all_metadata: &[DecryptedFileMetadata],
        metadata_changes: &[DecryptedFileMetadata],
    ) -> Result<(), CoreError> {
        // encrypt metadata
        let account = self.get_account()?;
        let all_metadata_with_changes_staged = all_metadata
            .stage(metadata_changes)
            .into_iter()
            .map(|(f, _)| f)
            .collect::<Vec<DecryptedFileMetadata>>();
        let all_metadata_encrypted =
            file_encryption_service::encrypt_metadata(&account, &all_metadata_with_changes_staged)?;

        for metadatum in metadata_changes {
            let encrypted_metadata = all_metadata_encrypted.find(metadatum.id)?;

            // perform insertion
            let new_doc = source == RepoSource::Local
                && metadatum.file_type == FileType::Document
                && self
                    .maybe_get_metadata(RepoSource::Local, metadatum.id)?
                    .is_none();

            match source {
                RepoSource::Local => {
                    self.local_metadata
                        .insert(encrypted_metadata.id, encrypted_metadata.clone());
                }
                RepoSource::Base => {
                    self.base_metadata
                        .insert(encrypted_metadata.id, encrypted_metadata.clone());
                }
            }

            println!("did it?");
            if new_doc {
                println!("This happened");
                self.insert_document(config, RepoSource::Local, metadatum, &[])?;
            }

            let opposite_metadata = match source.opposite() {
                RepoSource::Local => self.local_metadata.get(&encrypted_metadata.id),
                RepoSource::Base => self.base_metadata.get(&encrypted_metadata.id),
            };

            // remove local if local == base
            if let Some(opposite) = opposite_metadata {
                if utils::slices_equal(&opposite.name.hmac, &encrypted_metadata.name.hmac)
                    && opposite.parent == metadatum.parent
                    && opposite.deleted == metadatum.deleted
                {
                    self.local_metadata.delete(metadatum.id);
                }
            }

            // update root
            if metadatum.parent == metadatum.id {
                self.root.insert(OneKey {}, metadatum.id);
            }
        }

        Ok(())
    }

    pub fn maybe_get_metadata(
        &self, source: RepoSource, id: Uuid,
    ) -> Result<Option<DecryptedFileMetadata>, CoreError> {
        let all_metadata = self.get_all_metadata(source)?;
        Ok(all_metadata.maybe_find(id))
    }

    pub fn get_not_deleted_metadata(
        &self, source: RepoSource, id: Uuid,
    ) -> Result<DecryptedFileMetadata, CoreError> {
        self.maybe_get_not_deleted_metadata(source, id)
            .and_then(|f| f.ok_or(CoreError::FileNonexistent))
    }

    pub fn maybe_get_not_deleted_metadata(
        &self, source: RepoSource, id: Uuid,
    ) -> Result<Option<DecryptedFileMetadata>, CoreError> {
        let all_not_deleted_metadata = self.get_all_not_deleted_metadata(source)?;
        Ok(all_not_deleted_metadata.maybe_find(id))
    }

    pub fn get_children(&self, id: Uuid) -> Result<Vec<DecryptedFileMetadata>, CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        Ok(files.find_children(id))
    }

    pub fn get_and_get_children_recursively(
        &self, id: Uuid,
    ) -> Result<Vec<DecryptedFileMetadata>, CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        let file_and_descendants = files::find_with_descendants(&files, id)?;
        Ok(file_and_descendants)
    }

    pub fn delete_file(&mut self, config: &Config, id: Uuid) -> Result<(), CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        let file = files::apply_delete(&files, id)?;
        self.insert_metadatum(&config, RepoSource::Local, &file)?;
        self.prune_deleted(config)
    }

    /// Removes deleted files which are safe to delete. Call this function after a set of operations rather than in-between
    /// each operation because otherwise you'll prune e.g. a file that was moved out of a folder that was deleted.
    pub fn prune_deleted(&mut self, config: &Config) -> Result<(), CoreError> {
        // If a file is deleted or has a deleted ancestor, we say that it is deleted. Whether a file is deleted is specific
        // to the source (base or local). We cannot prune (delete from disk) a file in one source and not in the other in
        // order to preserve the semantics of having a file present on one, the other, or both (unmodified/new/modified).
        // For a file to be pruned, it must be deleted on both sources but also have no non-deleted descendants on either
        // source - otherwise, the metadata for those descendants can no longer be decrypted. For an example of a situation
        // where this is important, see the test prune_deleted_document_moved_from_deleted_folder_local_only.

        // find files deleted on base and local; new deleted local files are also eligible
        let all_base_metadata = self.get_all_metadata(RepoSource::Base)?;
        let deleted_base_metadata = all_base_metadata.filter_deleted()?;
        let all_local_metadata = self.get_all_metadata(RepoSource::Local)?;
        let deleted_local_metadata = all_local_metadata.filter_deleted()?;
        let deleted_both_metadata = deleted_base_metadata
            .into_iter()
            .filter(|f| deleted_local_metadata.maybe_find(f.id).is_some());
        let prune_eligible_metadata = deleted_local_metadata
            .iter()
            .filter_map(|f| {
                if all_base_metadata.maybe_find(f.id).is_none() {
                    Some(f.clone())
                } else {
                    None
                }
            })
            .chain(deleted_both_metadata)
            .collect::<Vec<DecryptedFileMetadata>>();

        // exclude files with not deleted descendants i.e. exclude files that are the ancestors of not deleted files
        let all_ids = all_base_metadata
            .iter()
            .chain(all_local_metadata.iter())
            .map(|f| f.id)
            .collect::<HashSet<Uuid>>();
        let not_deleted_either_ids = all_ids
            .into_iter()
            .filter(|&id| prune_eligible_metadata.maybe_find(id).is_none())
            .collect::<HashSet<Uuid>>();
        let ancestors_of_not_deleted_base_ids = not_deleted_either_ids
            .iter()
            .flat_map(|&id| files::find_ancestors(&all_base_metadata, id))
            .map(|f| f.id)
            .collect::<HashSet<Uuid>>();
        let ancestors_of_not_deleted_local_ids = not_deleted_either_ids
            .iter()
            .flat_map(|&id| files::find_ancestors(&all_local_metadata, id))
            .map(|f| f.id)
            .collect::<HashSet<Uuid>>();
        let deleted_both_without_deleted_descendants_ids =
            prune_eligible_metadata.into_iter().filter(|f| {
                !ancestors_of_not_deleted_base_ids.contains(&f.id)
                    && !ancestors_of_not_deleted_local_ids.contains(&f.id)
            });

        // remove files from disk
        for file in deleted_both_without_deleted_descendants_ids {
            self.delete_metadata(file.id);
            if file.file_type == FileType::Document {
                self.delete_document(config, file.id)?;
            }
        }
        Ok(())
    }

    fn delete_metadata(&mut self, id: Uuid) {
        self.local_metadata.delete(id);
        self.base_metadata.delete(id);
    }

    fn delete_document(&mut self, config: &Config, id: Uuid) -> Result<(), CoreError> {
        document_repo::delete(config, RepoSource::Local, id)?;
        document_repo::delete(config, RepoSource::Base, id)?;
        self.local_digest.delete(id);
        self.base_digest.delete(id);

        Ok(())
    }

    pub fn read_document(&self, config: &Config, id: Uuid) -> Result<DecryptedDocument, CoreError> {
        let all_metadata = self.get_all_metadata(RepoSource::Local)?;
        self.get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)
    }

    pub fn get_not_deleted_document(
        &self, config: &Config, source: RepoSource, metadata: &[DecryptedFileMetadata], id: Uuid,
    ) -> Result<DecryptedDocument, CoreError> {
        self.maybe_get_not_deleted_document(config, source, metadata, id)
            .and_then(|f| f.ok_or(CoreError::FileNonexistent))
    }

    pub fn maybe_get_not_deleted_document(
        &self, config: &Config, source: RepoSource, metadata: &[DecryptedFileMetadata], id: Uuid,
    ) -> Result<Option<DecryptedDocument>, CoreError> {
        if let Some(metadata) = metadata.filter_not_deleted()?.maybe_find(id) {
            maybe_get_document(config, source, &metadata)
        } else {
            Ok(None)
        }
    }

    pub fn save_document_to_disk(
        &self, config: &Config, id: Uuid, location: &str,
    ) -> Result<(), CoreError> {
        let all_metadata = self.get_all_metadata(RepoSource::Local)?;
        let document =
            self.get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)?;
        files::save_document_to_disk(&document, location.to_string())
    }

    pub fn rename_file(
        &mut self, config: &Config, id: Uuid, new_name: &str,
    ) -> Result<(), CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        let files = files.filter_not_deleted()?;
        let file = files::apply_rename(&files, id, new_name)?;
        self.insert_metadatum(config, RepoSource::Local, &file)
    }

    pub fn move_file(
        &mut self, config: &Config, id: Uuid, new_parent: Uuid,
    ) -> Result<(), CoreError> {
        let files = self.get_all_not_deleted_metadata(RepoSource::Local)?;
        let files = files.filter_not_deleted()?;
        let file = files::apply_move(&files, id, new_parent)?;
        self.insert_metadatum(config, RepoSource::Local, &file)
    }

    pub fn get_all_with_document_changes(&self, config: &Config) -> Result<Vec<Uuid>, CoreError> {
        let all = self.get_all_metadata(RepoSource::Local)?;
        let not_deleted = all.filter_not_deleted()?;
        let not_deleted_with_document_changes = not_deleted
            .into_iter()
            .map(|f| {
                document_repo::maybe_get(config, RepoSource::Local, f.id).map(|r| r.map(|_| f.id))
            })
            .collect::<Result<Vec<Option<Uuid>>, CoreError>>()?
            .into_iter()
            .flatten()
            .collect();
        Ok(not_deleted_with_document_changes)
    }

    pub fn get_all_metadata_with_encrypted_changes(
        &self, source: RepoSource, changes: &[EncryptedFileMetadata],
    ) -> Result<(Vec<DecryptedFileMetadata>, Vec<EncryptedFileMetadata>), CoreError> {
        let account = self.get_account()?;
        let base = self.base_metadata.get_all().values().cloned().collect_vec();
        let sourced = match source {
            RepoSource::Local => {
                let local = self
                    .local_metadata
                    .get_all()
                    .values()
                    .cloned()
                    .collect_vec();
                base.stage(&local).into_iter().map(|(f, _)| f).collect()
            }
            RepoSource::Base => base,
        };

        let staged = sourced
            .stage(changes)
            .into_iter()
            .map(|(f, _)| f)
            .collect::<Vec<EncryptedFileMetadata>>();

        let root = staged.find_root()?;
        let non_orphans = files::find_with_descendants(&staged, root.id)?;
        let mut staged_non_orphans = Vec::new();
        let mut encrypted_orphans = Vec::new();
        for f in staged {
            if non_orphans.maybe_find(f.id).is_some() {
                // only decrypt non-orphans
                staged_non_orphans.push(f)
            } else {
                // deleted orphaned files
                encrypted_orphans.push(f)
            }
        }

        Ok((
            file_encryption_service::decrypt_metadata(&account, &staged_non_orphans)?,
            encrypted_orphans,
        ))
    }

    pub fn get_all_metadata_state(
        &self,
    ) -> Result<Vec<RepoState<DecryptedFileMetadata>>, CoreError> {
        let account = self.get_account()?;
        let base_encrypted = self.base_metadata.get_all().values().cloned().collect_vec();
        let base = file_encryption_service::decrypt_metadata(&account, &base_encrypted)?;
        let local = {
            let local_encrypted = self
                .local_metadata
                .get_all()
                .values()
                .cloned()
                .collect_vec();
            let staged = base_encrypted
                .stage(&local_encrypted)
                .into_iter()
                .map(|(f, _)| f)
                .collect::<Vec<EncryptedFileMetadata>>();
            let decrypted = file_encryption_service::decrypt_metadata(&account, &staged)?;
            decrypted
                .into_iter()
                .filter(|d| local_encrypted.iter().any(|l| l.id == d.id))
                .collect::<Vec<DecryptedFileMetadata>>()
        };

        let new = local
            .iter()
            .filter(|&l| !base.iter().any(|b| l.id == b.id))
            .map(|l| RepoState::New(l.clone()));
        let unmodified = base
            .iter()
            .filter(|&b| !local.iter().any(|l| l.id == b.id))
            .map(|b| RepoState::Unmodified(b.clone()));
        let modified = base.iter().filter_map(|b| {
            local
                .maybe_find(b.id)
                .map(|l| RepoState::Modified { base: b.clone(), local: l })
        });

        Ok(new.chain(unmodified).chain(modified).collect())
    }

    /// Updates base metadata to match local metadata.
    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn promote_metadata(&mut self) -> Result<(), CoreError> {
        let base_metadata = self.base_metadata.get_all().into_values().collect_vec();
        let local_metadata = self.local_metadata.get_all().into_values().collect_vec();
        let staged_metadata = base_metadata.stage(&local_metadata);

        self.base_metadata.clear();

        for (metadata, _) in staged_metadata {
            self.base_metadata.insert(metadata.id, metadata.clone());
        }

        self.local_metadata.clear();

        Ok(())
    }

    pub fn get_all_metadata_changes(&self) -> Result<Vec<FileMetadataDiff>, CoreError> {
        let local = self.local_metadata.get_all().into_values().collect_vec();
        let base = self.base_metadata.get_all().into_values().collect_vec();

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
        let base_metadata = self.base_metadata.get_all().into_values().collect_vec();
        let local_metadata = self.local_metadata.get_all().into_values().collect_vec();
        let staged_metadata = base_metadata.stage(&local_metadata);
        let staged_everything = staged_metadata
            .into_iter()
            .map(|(f, _)| {
                Ok((
                    f.clone(),
                    match document_repo::maybe_get(config, RepoSource::Local, f.id)? {
                        Some(document) => Some(document),
                        None => document_repo::maybe_get(config, RepoSource::Base, f.id)?,
                    },
                    match self.local_digest.get(&f.id) {
                        Some(digest) => Some(digest),
                        None => self.base_digest.get(&f.id),
                    },
                ))
            })
            .collect::<Result<
                Vec<(EncryptedFileMetadata, Option<EncryptedDocument>, Option<Vec<u8>>)>,
                CoreError,
            >>()?;

        document_repo::delete_all(config, RepoSource::Base)?;
        self.base_digest.clear();

        for (metadata, maybe_document, maybe_digest) in staged_everything {
            if let Some(document) = maybe_document {
                document_repo::insert(config, RepoSource::Base, metadata.id, &document)?;
            }
            if let Some(digest) = maybe_digest {
                self.base_digest.insert(metadata.id, digest);
            }
        }

        document_repo::delete_all(config, RepoSource::Local)?;
        self.local_digest.clear();

        Ok(())
    }

    pub fn get_local_changes(&self, config: &Config) -> Result<Vec<Uuid>, CoreError> {
        Ok(self
            .get_all_metadata_changes()?
            .into_iter()
            .map(|f| f.id)
            .chain(self.get_all_with_document_changes(config)?.into_iter())
            .unique()
            .collect())
    }

    pub fn insert_metadata_both_repos(
        &mut self, config: &Config, base_metadata_changes: &[DecryptedFileMetadata],
        local_metadata_changes: &[DecryptedFileMetadata],
    ) -> Result<(), CoreError> {
        let base_metadata = self.get_all_metadata(RepoSource::Base)?;
        let local_metadata = self.get_all_metadata(RepoSource::Local)?;
        self.insert_metadata_given_decrypted_metadata(
            config,
            RepoSource::Base,
            &base_metadata,
            base_metadata_changes,
        )?;
        self.insert_metadata_given_decrypted_metadata(
            config,
            RepoSource::Local,
            &local_metadata,
            local_metadata_changes,
        )
    }

    pub fn get_metadata_state(
        &self, id: Uuid,
    ) -> Result<RepoState<DecryptedFileMetadata>, CoreError> {
        self.maybe_get_metadata_state(id)
            .and_then(|f| f.ok_or(CoreError::FileNonexistent))
    }

    pub fn maybe_get_metadata_state(
        &self, id: Uuid,
    ) -> Result<Option<RepoState<DecryptedFileMetadata>>, CoreError> {
        let all_metadata = self.get_all_metadata_state()?;
        Ok(files::maybe_find_state(&all_metadata, id))
    }

    pub fn get_all_document_state(
        &self, config: &Config,
    ) -> Result<Vec<RepoState<DecryptedDocument>>, CoreError> {
        let doc_metadata: Vec<RepoState<DecryptedFileMetadata>> = self
            .get_all_metadata_state()?
            .into_iter()
            .filter(|r| r.clone().local().file_type == FileType::Document)
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
    config: &Config, source: RepoSource, metadata: &[DecryptedFileMetadata], id: Uuid,
) -> Result<DecryptedDocument, CoreError> {
    maybe_get_not_deleted_document(config, source, metadata, id)
        .and_then(|f| f.ok_or(CoreError::FileNonexistent))
}

pub fn maybe_get_not_deleted_document(
    config: &Config, source: RepoSource, metadata: &[DecryptedFileMetadata], id: Uuid,
) -> Result<Option<DecryptedDocument>, CoreError> {
    if let Some(metadata) = metadata.filter_not_deleted()?.maybe_find(id) {
        maybe_get_document(config, source, &metadata)
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
