use crate::model::core_file::{Base, Local};
use crate::model::repo::RepoSource;
use crate::repo::document_repo;
use crate::service::file_compression_service;
use crate::CoreError::RootNonexistent;
use crate::{Config, CoreError, OneKey, RequestContext};
use itertools::Itertools;
use lockbook_shared::crypto::DecryptedDocument;
use lockbook_shared::crypto::EncryptedDocument;
use lockbook_shared::file::File;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::FileType;
use lockbook_shared::lazy::LazyTree;
use lockbook_shared::tree_like::{Stagable, TreeLike};
use lockbook_shared::utils;
use sha2::Digest;
use sha2::Sha256;
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

impl<'a> RequestContext<'a, 'a> {
    pub fn create_file(
        &'a mut self, name: &str, parent: Uuid, file_type: FileType,
    ) -> Result<File, CoreError> {
        let pub_key = self.get_public_key()?;
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let (mut tree, id) = Base(&mut self.tx.base_metadata)
            .stage(Local(&mut self.tx.local_metadata))
            .to_lazy()
            .create(parent, name, file_type, account, &pub_key)?;

        let ui_file = tree.finalize(id, account)?;

        Ok(ui_file)
    }

    // pub fn rename_file(&'a mut self, id: Uuid, new_name: &str) -> Result<(), CoreError> {
    //     let account = self.get_account()?;
    //     self.tree().rename(id, new_name, account)?;
    //     Ok(())
    // }
    //
    // pub fn move_file(&'a mut self, id: Uuid, new_parent: Uuid) -> Result<(), CoreError> {
    //     let account = self.get_account()?;
    //     self.tree().move_file(id, new_parent, account)?;
    //     Ok(())
    // }
    //
    // pub fn delete(&'a mut self, id: Uuid) -> Result<(), CoreError> {
    //     let account = self.get_account()?;
    //     self.tree().delete(id, account)?;
    //     Ok(())
    // }
    //
    // pub fn root_id(&self) -> Result<&Uuid, CoreError> {
    //     self.tx.root.get(&OneKey).ok_or(RootNonexistent)
    // }
    //
    // /// Adds or updates the content of a document on disk.
    // /// Disk optimization opportunity: this function needlessly writes to disk when setting local content = base content.
    // pub fn insert_document(
    //     &'a mut self, source: RepoSource, id: Uuid, document: &[u8],
    // ) -> Result<(), CoreError> {
    //     let mut tree = self.tree();
    //     let account = self.get_account()?;
    //     let config = self.config;
    //     // check that document exists and is a document
    //     let metadata = tree.find(id)?;
    //
    //     if tree.calculate_deleted(id)? {
    //         return Err(CoreError::FileParentNonexistent);
    //     }
    //
    //     if metadata.is_folder() {
    //         return Err(CoreError::FileNotDocument);
    //     }
    //
    //     // encrypt document and compute digest
    //     let digest = Sha256::digest(document);
    //     let compressed_document = file_compression_service::compress(document)?;
    //     let encrypted_document = tree.encrypt_document(id, &compressed_document, account)?;
    //
    //     // perform insertions
    //     document_repo::insert(self.config, source, metadata.id, &encrypted_document)?;
    //     match source {
    //         RepoSource::Local => {
    //             self.tx.local_digest.insert(metadata.id, digest.to_vec());
    //         }
    //         RepoSource::Base => {
    //             self.tx.base_digest.insert(metadata.id, digest.to_vec());
    //         }
    //     }
    //
    //     let opposite_digest = match source.opposite() {
    //         RepoSource::Local => self.tx.local_digest.get(&metadata.id),
    //         RepoSource::Base => self.tx.base_digest.get(&metadata.id),
    //     };
    //
    //     // remove local if local == base
    //     if let Some(opposite) = opposite_digest {
    //         if utils::slices_equal(&opposite, &digest) {
    //             self.tx.local_digest.delete(metadata.id);
    //             document_repo::delete(config, RepoSource::Local, metadata.id)?;
    //         }
    //     }
    //
    //     Ok(())
    // }
    //
    //     #[instrument(level = "debug", skip_all, err(Debug))]
    //     pub fn prune_deleted(&mut self, config: &Config) -> Result<(), CoreError> {
    //         // If a file is deleted or has a deleted ancestor, we say that it is deleted. Whether a file is deleted is specific
    //         // to the source (base or local). We cannot prune (delete from disk) a file in one source and not in the other in
    //         // order to preserve the semantics of having a file present on one, the other, or both (unmodified/new/modified).
    //         // For a file to be pruned, it must be deleted on both sources but also have no non-deleted descendants on either
    //         // source - otherwise, the metadata for those descendants can no longer be decrypted. For an example of a situation
    //         // where this is important, see the test prune_deleted_document_moved_from_deleted_folder_local_only.
    //
    //         // find files deleted on base and local; new deleted local files are also eligible
    //         let all_base_metadata = self.get_all_metadata(RepoSource::Base)?;
    //         let deleted_base_metadata = all_base_metadata.deleted_status()?.deleted;
    //         let all_local_metadata = self.get_all_metadata(RepoSource::Local)?;
    //         let deleted_local_metadata = all_local_metadata.deleted_status()?.deleted;
    //         let deleted_both_metadata = deleted_base_metadata
    //             .into_iter()
    //             .filter(|id| deleted_local_metadata.contains(id));
    //         let prune_eligible_ids =
    //             deleted_local_metadata
    //                 .iter()
    //                 .filter_map(|id| {
    //                     if all_base_metadata.maybe_find(*id).is_none() {
    //                         Some(*id)
    //                     } else {
    //                         None
    //                     }
    //                 })
    //                 .chain(deleted_both_metadata)
    //                 .collect::<HashSet<Uuid>>();
    //
    //         // exclude files with not deleted descendants i.e. exclude files that are the ancestors of not deleted files
    //         let all_ids = all_base_metadata
    //             .keys()
    //             .chain(all_local_metadata.keys())
    //             .cloned()
    //             .collect::<HashSet<Uuid>>();
    //         let not_deleted_either_ids = all_ids
    //             .into_iter()
    //             .filter(|id| !prune_eligible_ids.contains(id))
    //             .collect::<HashSet<Uuid>>();
    //         let ancestors_of_not_deleted_base_ids = not_deleted_either_ids
    //             .iter()
    //             .flat_map(|&id| files::find_ancestors(&all_base_metadata, id).into_keys())
    //             .collect::<HashSet<Uuid>>();
    //         let ancestors_of_not_deleted_local_ids = not_deleted_either_ids
    //             .iter()
    //             .flat_map(|&id| files::find_ancestors(&all_local_metadata, id).into_keys())
    //             .collect::<HashSet<Uuid>>();
    //         let deleted_both_without_deleted_descendants_ids =
    //             prune_eligible_ids.into_iter().filter(|id| {
    //                 !ancestors_of_not_deleted_base_ids.contains(id)
    //                     && !ancestors_of_not_deleted_local_ids.contains(id)
    //             });
    //
    //         // remove files from disk
    //         for id in deleted_both_without_deleted_descendants_ids {
    //             self.tx.local_metadata.delete(id);
    //             self.tx.base_metadata.delete(id);
    //             if all_local_metadata.find_ref(id)?.is_document() {
    //                 self.delete_document(config, id)?;
    //             }
    //         }
    //         Ok(())
    //     }
    //
    //     fn delete_document(&mut self, config: &Config, id: Uuid) -> Result<(), CoreError> {
    //         document_repo::delete(config, RepoSource::Local, id)?;
    //         document_repo::delete(config, RepoSource::Base, id)?;
    //         self.tx.local_digest.delete(id);
    //         self.tx.base_digest.delete(id);
    //
    //         Ok(())
    //     }
    //
    //     pub fn maybe_get_not_deleted_document(
    //         &self, config: &Config, source: RepoSource, metadata: &DecryptedFiles, id: Uuid,
    //     ) -> Result<Option<DecryptedDocument>, CoreError> {
    //         if let Some(metadata) = metadata.maybe_find(id) {
    //             maybe_get_document(config, source, &metadata)
    //         } else {
    //             Ok(None)
    //         }
    //     }
    //
    //     pub fn get_all_with_document_changes(
    //         &mut self, config: &Config,
    //     ) -> Result<Vec<Uuid>, CoreError> {
    //         let not_deleted = self.get_all_not_deleted_metadata(RepoSource::Local)?;
    //         let not_deleted_with_document_changes = not_deleted
    //             .documents()
    //             .iter()
    //             .map(|id| {
    //                 document_repo::maybe_get(config, RepoSource::Local, *id).map(|r| r.map(|_| *id))
    //             })
    //             .collect::<Result<Vec<Option<Uuid>>, CoreError>>()?
    //             .into_iter()
    //             .flatten()
    //             .collect();
    //         Ok(not_deleted_with_document_changes)
    //     }
    //
    //     pub fn get_all_metadata_with_encrypted_changes(
    //         &mut self, source: RepoSource, changes: &EncryptedFiles,
    //     ) -> Result<(DecryptedFiles, EncryptedFiles), CoreError> {
    //         let account = self.get_account()?;
    //         let base = self.tx.base_metadata.get_all();
    //         let sourced = match source {
    //             RepoSource::Local => {
    //                 let local = self.tx.local_metadata.get_all();
    //                 base.stage_with_source(&local)
    //                     .into_iter()
    //                     .map(|(id, (f, _))| (id, f))
    //                     .collect()
    //             }
    //             RepoSource::Base => base,
    //         };
    //
    //         let staged = sourced
    //             .stage_with_source(changes)
    //             .into_iter()
    //             .map(|(id, (f, _))| (id, f))
    //             .collect::<EncryptedFiles>();
    //
    //         let root = match self.tx.root.get(&OneKey) {
    //             Some(id) => staged.find(id),
    //             None => staged.find_root(),
    //         }?;
    //         let non_orphans = files::find_with_descendants(&staged, root.id)?;
    //         let mut staged_non_orphans = HashMap::new();
    //         let mut encrypted_orphans = HashMap::new();
    //         for (id, f) in staged {
    //             if non_orphans.maybe_find(id).is_some() {
    //                 // only decrypt non-orphans
    //                 staged_non_orphans.push(f);
    //             } else {
    //                 // deleted orphaned files
    //                 encrypted_orphans.push(f);
    //             }
    //         }
    //
    //         Ok((
    //             file_encryption_service::decrypt_metadata(
    //                 &account,
    //                 &staged_non_orphans,
    //                 &mut self.data_cache.key_cache,
    //             )?,
    //             encrypted_orphans,
    //         ))
    //     }
    //
    //     pub fn get_all_metadata_state(&mut self) -> Result<Vec<RepoState<CoreFile>>, CoreError> {
    //         let account = self.get_account()?;
    //         let base_encrypted = self.tx.base_metadata.get_all();
    //         let base = file_encryption_service::decrypt_metadata(
    //             &account,
    //             &base_encrypted,
    //             &mut self.data_cache.key_cache,
    //         )?;
    //         let local = {
    //             let local_encrypted = self.tx.local_metadata.get_all();
    //             let staged = base_encrypted
    //                 .stage_with_source(&local_encrypted)
    //                 .into_iter()
    //                 .map(|(id, (f, _))| (id, f))
    //                 .collect::<EncryptedFiles>();
    //             let decrypted = file_encryption_service::decrypt_metadata(
    //                 &account,
    //                 &staged,
    //                 &mut self.data_cache.key_cache,
    //             )?;
    //             decrypted
    //                 .into_iter()
    //                 .filter(|(d_id, _)| local_encrypted.keys().any(|l_id| l_id == d_id))
    //                 .collect::<DecryptedFiles>()
    //         };
    //
    //         let new = local
    //             .values()
    //             .filter(|l| !base.values().any(|b| l.id == b.id))
    //             .map(|l| RepoState::New(l.clone()));
    //         let unmodified = base
    //             .values()
    //             .filter(|b| !local.values().any(|l| l.id == b.id))
    //             .map(|b| RepoState::Unmodified(b.clone()));
    //         let modified = base.values().filter_map(|b| {
    //             local
    //                 .maybe_find(b.id)
    //                 .map(|l| RepoState::Modified { base: b.clone(), local: l })
    //         });
    //
    //         Ok(new.chain(unmodified).chain(modified).collect())
    //     }
    //
    //     /// Updates base metadata to match local metadata.
    //     #[instrument(level = "debug", skip_all, err(Debug))]
    //     pub fn promote_metadata(&mut self) -> Result<(), CoreError> {
    //         let base_metadata = self.tx.base_metadata.get_all();
    //         let local_metadata = self.tx.local_metadata.get_all();
    //         let staged_metadata = base_metadata.stage_with_source(&local_metadata);
    //
    //         self.tx.base_metadata.clear();
    //
    //         for (metadata, _) in staged_metadata.values() {
    //             self.tx.base_metadata.insert(metadata.id, metadata.clone());
    //         }
    //
    //         self.tx.local_metadata.clear();
    //
    //         Ok(())
    //     }
    //
    //     pub fn get_all_metadata_changes(&self) -> Result<Vec<FileDiff>, CoreError> {
    //         let local = self.tx.local_metadata.get_all().into_values().collect_vec();
    //         let base = self.tx.base_metadata.get_all().into_values().collect_vec();
    //
    //         let new = local
    //             .iter()
    //             .filter(|l| !base.iter().any(|r| r.id == l.id))
    //             .map(FileDiff::new);
    //         let changed = local
    //             .iter()
    //             .filter_map(|l| base.iter().find(|r| r.id == l.id).map(|r| (l, r)))
    //             .map(|(l, r)| FileDiff::from);
    //
    //         Ok(new.chain(changed).collect())
    //     }
    //
    //     /// Updates base documents to match local documents.
    //     #[instrument(level = "debug", skip_all, err(Debug))]
    //     pub fn promote_documents(&mut self, config: &Config) -> Result<(), CoreError> {
    //         let base_metadata = self.tx.base_metadata.get_all();
    //         let local_metadata = self.tx.local_metadata.get_all();
    //         let staged_metadata = base_metadata.stage_with_source(&local_metadata);
    //         let staged_everything =
    //             staged_metadata
    //                 .values()
    //                 .map(|(f, _)| {
    //                     Ok((
    //                         f.clone(),
    //                         match document_repo::maybe_get(config, RepoSource::Local, f.id)? {
    //                             Some(document) => Some(document),
    //                             None => document_repo::maybe_get(config, RepoSource::Base, f.id)?,
    //                         },
    //                         match self.tx.local_digest.get(&f.id) {
    //                             Some(digest) => Some(digest),
    //                             None => self.tx.base_digest.get(&f.id),
    //                         },
    //                     ))
    //                 })
    //                 .collect::<Result<
    //                     Vec<(FileMetadata, Option<EncryptedDocument>, Option<Vec<u8>>)>,
    //                     CoreError,
    //                 >>()?;
    //
    //         document_repo::delete_all(config, RepoSource::Base)?;
    //         self.tx.base_digest.clear();
    //
    //         for (metadata, maybe_document, maybe_digest) in staged_everything {
    //             if let Some(document) = maybe_document {
    //                 document_repo::insert(config, RepoSource::Base, metadata.id, &document)?;
    //             }
    //             if let Some(digest) = maybe_digest {
    //                 self.tx.base_digest.insert(metadata.id, digest);
    //             }
    //         }
    //
    //         document_repo::delete_all(config, RepoSource::Local)?;
    //         self.tx.local_digest.clear();
    //
    //         Ok(())
    //     }
    //
    //     pub fn get_local_changes(&mut self, config: &Config) -> Result<Vec<Uuid>, CoreError> {
    //         Ok(self
    //             .get_all_metadata_changes()?
    //             .into_iter()
    //             .map(|f| f.id)
    //             .chain(self.get_all_with_document_changes(config)?.into_iter())
    //             .unique()
    //             .collect())
    //     }
    //
    //     pub fn insert_metadata_both_repos(
    //         &mut self, config: &Config, base_metadata_changes: &DecryptedFiles,
    //         local_metadata_changes: &DecryptedFiles,
    //     ) -> Result<(), CoreError> {
    //         let base_metadata = self.get_all_metadata(RepoSource::Base)?;
    //         let local_metadata = self.get_all_metadata(RepoSource::Local)?;
    //         self.insert_metadata_given_decrypted_metadata(
    //             RepoSource::Base,
    //             &base_metadata,
    //             base_metadata_changes,
    //         )?;
    //         self.insert_metadata_given_decrypted_metadata(
    //             RepoSource::Local,
    //             &local_metadata,
    //             local_metadata_changes,
    //         )?;
    //         self.insert_new_docs(config, &local_metadata, local_metadata_changes)
    //     }
    //
    //     pub fn get_metadata_state(&mut self, id: Uuid) -> Result<RepoState<CoreFile>, CoreError> {
    //         self.maybe_get_metadata_state(id)
    //             .and_then(|f| f.ok_or(CoreError::FileNonexistent))
    //     }
    //
    //     pub fn maybe_get_metadata_state(
    //         &mut self, id: Uuid,
    //     ) -> Result<Option<RepoState<CoreFile>>, CoreError> {
    //         let all_metadata = self.get_all_metadata_state()?;
    //         Ok(files::maybe_find_state(&all_metadata, id))
    //     }
    //
    //     pub fn get_all_document_state(
    //         &mut self, config: &Config,
    //     ) -> Result<Vec<RepoState<DecryptedDocument>>, CoreError> {
    //         let doc_metadata: Vec<RepoState<CoreFile>> = self
    //             .get_all_metadata_state()?
    //             .into_iter()
    //             .filter(|r| r.clone().local().is_document())
    //             .collect();
    //         let mut result = Vec::new();
    //         for doc_metadatum in doc_metadata {
    //             if let Some(doc_state) = maybe_get_document_state(config, &doc_metadatum)? {
    //                 result.push(doc_state);
    //             }
    //         }
    //         Ok(result)
    //     }
    // }
    //
    // pub fn get_not_deleted_document(
    //     config: &Config, source: RepoSource, metadata: &DecryptedFiles, id: Uuid,
    // ) -> Result<DecryptedDocument, CoreError> {
    //     maybe_get_not_deleted_document(config, source, metadata, id)
    //         .and_then(|f| f.ok_or(CoreError::FileNonexistent))
    // }
    //
    // pub fn maybe_get_not_deleted_document(
    //     config: &Config, source: RepoSource, metadata: &DecryptedFiles, id: Uuid,
    // ) -> Result<Option<DecryptedDocument>, CoreError> {
    //     let maybe_doc_metadata = metadata
    //         .deleted_status()?
    //         .not_deleted
    //         .get(&id)
    //         .and_then(|id| metadata.get(id));
    //
    //     if let Some(metadata) = maybe_doc_metadata {
    //         maybe_get_document(config, source, metadata)
    //     } else {
    //         Ok(None)
    //     }
    // }
    //
    // pub fn get_document(
    //     config: &Config, source: RepoSource, metadata: &CoreFile,
    // ) -> Result<DecryptedDocument, CoreError> {
    //     maybe_get_document(config, source, metadata).and_then(|f| f.ok_or(CoreError::FileNonexistent))
    // }
    //
    // pub fn maybe_get_document(
    //     config: &Config, source: RepoSource, metadata: &CoreFile,
    // ) -> Result<Option<DecryptedDocument>, CoreError> {
    //     if metadata.file_type != FileType::Document {
    //         return Err(CoreError::FileNotDocument);
    //     }
    //     let maybe_encrypted_document = match source {
    //         RepoSource::Local => {
    //             match document_repo::maybe_get(config, RepoSource::Local, metadata.id)? {
    //                 Some(local) => Some(local),
    //                 None => document_repo::maybe_get(config, RepoSource::Base, metadata.id)?,
    //             }
    //         }
    //         RepoSource::Base => document_repo::maybe_get(config, RepoSource::Base, metadata.id)?,
    //     };
    //
    //     Ok(match maybe_encrypted_document {
    //         None => None,
    //         Some(encrypted_document) => {
    //             let compressed_document =
    //                 file_encryption_service::decrypt_document(&encrypted_document, metadata)?;
    //             let document = file_compression_service::decompress(&compressed_document)?;
    //             Some(document)
    //         }
    //     })
}

// pub fn maybe_get_document_state(
//     config: &Config, metadata: &RepoState<CoreFile>,
// ) -> Result<Option<RepoState<DecryptedDocument>>, CoreError> {
//     if metadata.clone().local().file_type != FileType::Document {
//         return Err(CoreError::FileNotDocument);
//     }
//     let id = metadata.clone().local().id;
//
//     let base = if let Some(base_metadata) = metadata.clone().base() {
//         match document_repo::maybe_get(config, RepoSource::Base, id)? {
//             None => None,
//             Some(encrypted_document) => {
//                 let compressed_document =
//                     file_encryption_service::decrypt_document(&encrypted_document, &base_metadata)?;
//                 let document = file_compression_service::decompress(&compressed_document)?;
//                 Some(document)
//             }
//         }
//     } else {
//         None
//     };
//     let local = match document_repo::maybe_get(config, RepoSource::Local, id)? {
//         None => None,
//         Some(encrypted_document) => {
//             let compressed_document = file_encryption_service::decrypt_document(
//                 &encrypted_document,
//                 &metadata.clone().local(),
//             )?;
//             let document = file_compression_service::decompress(&compressed_document)?;
//             Some(document)
//         }
//     };
//     Ok(RepoState::from_local_and_base(local, base))
// }
