use crate::model::repo::RepoSource;
use crate::repo::document_repo;
use crate::repo::schema::OneKey;
use crate::service::{api_service, file_service};
use crate::CoreResult;
use crate::{Config, CoreError, RequestContext};
use lockbook_shared::account::Account;
use lockbook_shared::api::{FileMetadataUpsertsRequest, GetDocumentRequest, GetUpdatesRequest};
use lockbook_shared::clock::get_time;
use lockbook_shared::compression_service;
use lockbook_shared::crypto::DecryptedDocument;
use lockbook_shared::crypto::EncryptedDocument;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::DocumentHmac;
use lockbook_shared::file_metadata::FileMetadata;
use lockbook_shared::filename::DocumentType;
use lockbook_shared::filename::NameComponents;
use lockbook_shared::lazy::LazyTree;
use lockbook_shared::secret_filename::SecretFileName;
use lockbook_shared::signed_file::SignedFile;
use lockbook_shared::tree_like::Stagable;
use lockbook_shared::tree_like::TreeLike;
use serde::Serialize;
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Serialize, Clone)]
pub struct WorkCalculated {
    pub work_units: Vec<WorkUnit>,
    pub most_recent_update_from_server: u64,
}

pub struct SyncProgress {
    pub total: usize,
    pub progress: usize,
    pub current_work_unit: ClientWorkUnit,
}

enum ResolvedDocument {
    Merged {
        remote_metadata: CoreFile,
        remote_document: DecryptedDocument,
        merged_metadata: CoreFile,
        merged_document: DecryptedDocument,
    },
    Copied {
        remote_metadata: CoreFile,
        remote_document: DecryptedDocument,
        copied_local_metadata: CoreFile,
        copied_local_document: DecryptedDocument,
    },
}

impl fmt::Debug for ResolvedDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolvedDocument::Merged {
                remote_metadata,
                remote_document,
                merged_metadata,
                merged_document,
            } => f
                .debug_struct("ResolvedDocument::Merged")
                .field("remote_metadata", remote_metadata)
                .field("remote_document", &String::from_utf8_lossy(remote_document))
                .field("merged_metadata", merged_metadata)
                .field("merged_document", &String::from_utf8_lossy(merged_document))
                .finish(),
            ResolvedDocument::Copied {
                remote_metadata,
                remote_document,
                copied_local_metadata,
                copied_local_document,
            } => f
                .debug_struct("ResolvedDocument::Copied")
                .field("remote_metadata", remote_metadata)
                .field("remote_document", &String::from_utf8_lossy(remote_document))
                .field("copied_local_metadata", copied_local_metadata)
                .field("copied_local_document", &String::from_utf8_lossy(copied_local_document))
                .finish(),
        }
    }
}

fn should_pull_document(
    maybe_base_hmac: Option<&DocumentHmac>, maybe_remote_hmac: Option<&DocumentHmac>,
) -> bool {
    match (maybe_base_hmac, maybe_remote_hmac) {
        (_, None) => false,
        (None, _) => true,
        (Some(base_hmac), Some(remote_hmac)) => base_hmac != remote_hmac,
    }
}

/// Returns the 3-way merge of any comparable value; returns `resolution` in the event of a conflict.
///
/// # Examples
///
/// ```
/// let (base, local, remote) = ("hello", "hello local", "hello remote");
/// let result = merge(base, local, remote, remote);
/// assert_eq!(result, "hello remote");
/// ```
pub fn three_way_merge<'a, T: Eq + ?Sized>(
    base: &'a T, remote: &'a T, local: &'a T, resolution: &'a T,
) -> &'a T {
    let remote_changed = remote != base;
    let local_changed = local != base;
    match (remote_changed, local_changed) {
        (false, false) => base,
        (false, true) => local,
        (true, false) => remote,
        (true, true) => resolution,
    }
}

enum SyncProgressOperation {
    IncrementTotalWork(usize),
    StartWorkUnit(ClientWorkUnit),
}

impl RequestContext<'_, '_> {
    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn sync<F: Fn(SyncProgress)>(
        &mut self, config: &Config, maybe_update_sync_progress: Option<F>,
    ) -> Result<(), CoreError> {
        let mut sync_progress_total = 4 + self.get_all_with_document_changes(config)?.len(); // 3 metadata pulls + 1 metadata push + doc pushes
        let mut sync_progress = 0;
        let mut update_sync_progress = |op: SyncProgressOperation| match op {
            SyncProgressOperation::IncrementTotalWork(inc) => sync_progress_total += inc,
            SyncProgressOperation::StartWorkUnit(work_unit) => {
                if let Some(ref update_sync_progress) = maybe_update_sync_progress {
                    update_sync_progress(SyncProgress {
                        total: sync_progress_total,
                        progress: sync_progress,
                        current_work_unit: work_unit,
                    })
                }
                sync_progress += 1;
            }
        };

        self.pull(config, &mut update_sync_progress)?;
        self.prune_deleted(config)?;
        self.push_metadata(config, &mut update_sync_progress)?;
        self.pull(config, &mut update_sync_progress)?;
        self.push_documents(config, &mut update_sync_progress)?;
        self.pull(config, &mut update_sync_progress)?;
        self.prune_deleted(config)?;
        self.tx.last_synced.insert(OneKey {}, get_time().0);
        Ok(())
    }

    /// Pulls remote changes and constructs a changeset Merge such that Stage<Stage<Stage<Base, Remote>, Local>, Merge> is valid.
    /// Promotes Base to Stage<Base, Remote> and Local to Stage<Local, Merge>
    fn pull<F>(&mut self, config: &Config, update_sync_progress: &mut F) -> Result<(), CoreError>
    where
        F: FnMut(SyncProgressOperation),
    {
        // fetch metadata updates
        let account = self.get_account()?;
        let last_synced = self
            .tx
            .last_synced
            .get(&OneKey {})
            .map(|&i| i)
            .unwrap_or_default() as u64;
        let remote_changes = api_service::request(
            // todo: if this doesn't need to be mut, prune is broken
            &account,
            GetUpdatesRequest { since_metadata_version: last_synced },
        )?
        .file_metadata;

        // prune prunable files
        {
            let mut staged = self
                .tx
                .base_metadata
                .stage(remote_changes)
                .stage(&mut self.tx.local_metadata)
                .to_lazy();
            for id in staged.prunable_ids()? {
                staged.remove(id);
            }
        }

        // track work
        {
            let base = self.tx.base_metadata.to_lazy();
            let mut num_documents_to_pull = 0;
            for id in remote_changes.owned_ids() {
                let maybe_base_hmac = base.maybe_find(&id).map(|f| f.document_hmac()).flatten();
                let maybe_remote_hmac = remote_changes.find(&id)?.document_hmac();
                if should_pull_document(maybe_base_hmac, maybe_remote_hmac) {
                    num_documents_to_pull += 1;
                }
            }
            update_sync_progress(SyncProgressOperation::IncrementTotalWork(num_documents_to_pull));
        }

        // fetch document updates and local documents for merge
        let mut base_documents = HashMap::new();
        let mut remote_document_changes = HashMap::new();
        let mut local_document_changes = HashMap::new();
        {
            for id in remote_changes.owned_ids() {
                if let Some(remote_document_change) =
                    get_document(account, &mut self.tx.base_metadata, remote_changes, id)?
                {
                    // todo: update progress
                    document_repo::maybe_get(config, RepoSource::Base, &id)?
                        .map(|d| base_documents.insert(id, d));
                    remote_document_changes.insert(id, remote_document_change);
                    document_repo::maybe_get(config, RepoSource::Local, &id)?
                        .map(|d| local_document_changes.insert(id, d));
                }
            }
        };

        // merge
        let (merge_changes, merge_document_changes) = get_merge_changes(
            &account,
            &mut self.tx.base_metadata,
            remote_changes,
            &mut self.tx.local_metadata,
            &base_documents,
            &remote_document_changes,
            &local_document_changes,
        )?;

        // promote
        self.tx
            .base_metadata
            .stage(remote_changes)
            .to_lazy()
            .promote();
        self.tx
            .local_metadata
            .stage(merge_changes)
            .to_lazy()
            .promote();
        for (id, document) in remote_document_changes {
            document_repo::insert(self.config, RepoSource::Base, id, &document);
        }
        for (id, document) in merge_document_changes {
            document_repo::insert(self.config, RepoSource::Local, id, &document);
        }

        Ok(())
    }

    /// Updates remote and base metadata to local.
    #[instrument(level = "debug", skip_all, err(Debug))]
    fn push_metadata<F>(
        &mut self, _config: &Config, update_sync_progress: &mut F,
    ) -> Result<(), CoreError>
    where
        F: FnMut(SyncProgressOperation),
    {
        let account = &self.get_account()?;
        update_sync_progress(SyncProgressOperation::StartWorkUnit(ClientWorkUnit::PushMetadata));

        // update remote to local (metadata)
        let metadata_changes = self.get_all_metadata_changes()?;
        if !metadata_changes.is_empty() {
            api_service::request(account, FileMetadataUpsertsRequest { updates: metadata_changes })
                .map_err(CoreError::from)?;
        }

        // update base to local
        self.promote_metadata()?;

        Ok(())
    }

    /// Updates remote and base files to local.
    #[instrument(level = "debug", skip_all, err(Debug))]
    fn push_documents<F>(
        &mut self, config: &Config, update_sync_progress: &mut F,
    ) -> Result<(), CoreError>
    where
        F: FnMut(SyncProgressOperation),
    {
        let account = &self.get_account()?;
        for id in self.get_all_with_document_changes(config)? {
            let mut local_metadata = self.get_metadata(RepoSource::Local, id)?;
            let local_content =
                file_service::get_document(config, RepoSource::Local, &local_metadata)?;
            let encrypted_content = file_encryption_service::encrypt_document(
                &file_compression_service::compress(&local_content)?,
                &local_metadata,
            )?;

            update_sync_progress(SyncProgressOperation::StartWorkUnit(
                ClientWorkUnit::PushDocument(local_metadata.decrypted_name.clone()),
            ));

            // update remote to local (document)
            local_metadata.content_version = api_service::request(
                account,
                ChangeDocumentContentRequest {
                    id,
                    old_metadata_version: local_metadata.metadata_version,
                    new_content: encrypted_content,
                },
            )
            .map_err(CoreError::from)?
            .new_content_version;

            // save content version change
            let mut base_metadata = self.get_metadata(RepoSource::Base, id)?;
            base_metadata.content_version = local_metadata.content_version;
            self.insert_metadatum(config, RepoSource::Local, &local_metadata)?;
            self.insert_metadatum(config, RepoSource::Base, &base_metadata)?;
        }

        // update base to local
        self.promote_documents(config)?;

        Ok(())
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub fn calculate_work(&mut self, config: &Config) -> Result<WorkCalculated, CoreError> {
        let account = &self.get_account()?;
        let base_metadata = self.get_all_metadata(RepoSource::Base)?;
        let base_max_metadata_version = base_metadata
            .values()
            .map(|f| f.metadata_version)
            .max()
            .unwrap_or(0);

        let server_updates = api_service::request(
            account,
            GetUpdatesRequest { since_metadata_version: base_max_metadata_version },
        )
        .map_err(CoreError::from)?
        .file_metadata
        .iter()
        .map(|f| (f.id, f.clone()))
        .collect();

        self.calculate_work_from_updates(config, &server_updates, base_max_metadata_version)
    }

    fn calculate_work_from_updates(
        &mut self, config: &Config, server_updates: &EncryptedFiles, mut last_sync: u64,
    ) -> Result<WorkCalculated, CoreError> {
        let mut work_units: Vec<WorkUnit> = vec![];
        let (all_metadata, _) =
            self.get_all_metadata_with_encrypted_changes(RepoSource::Local, server_updates)?;
        for (&id, metadata) in server_updates {
            // skip filtered changes
            if all_metadata.maybe_find(id).is_none() {
                continue;
            }

            if metadata.metadata_version > last_sync {
                last_sync = metadata.metadata_version;
            }

            match self.maybe_get_metadata(RepoSource::Local, id)? {
                None => {
                    if !metadata.is_deleted {
                        // no work for files we don't have that have been deleted
                        work_units.push(WorkUnit::ServerChange { metadata: all_metadata.find(id)? })
                    }
                }
                Some(local_metadata) => {
                    if metadata.metadata_version != local_metadata.metadata_version {
                        work_units.push(WorkUnit::ServerChange { metadata: all_metadata.find(id)? })
                    }
                }
            };
        }

        work_units.sort_by(|f1, f2| {
            f1.get_metadata()
                .metadata_version
                .cmp(&f2.get_metadata().metadata_version)
        });

        for file_diff in self.get_all_metadata_changes()? {
            let metadata = self.get_metadata(RepoSource::Local, file_diff.id)?;
            work_units.push(WorkUnit::LocalChange { metadata });
        }
        for doc_id in self.get_all_with_document_changes(config)? {
            let metadata = self.get_metadata(RepoSource::Local, doc_id)?;
            work_units.push(WorkUnit::LocalChange { metadata });
        }

        Ok(WorkCalculated { work_units, most_recent_update_from_server: last_sync })
    }
}

pub fn suggest_non_conflicting_filename(
    id: Uuid, files: &DecryptedFiles, staged_changes: &DecryptedFiles,
) -> Result<String, CoreError> {
    let files: DecryptedFiles = files
        .stage_with_source(staged_changes)
        .into_iter()
        .map(|(id, (f, _))| (id, f))
        .collect::<DecryptedFiles>();

    let file = files.find(id)?;
    let sibblings = files.find_children(file.parent);

    let mut new_name = NameComponents::from(&file.decrypted_name).generate_next();
    loop {
        if !sibblings
            .values()
            .any(|f| f.decrypted_name == new_name.to_name())
        {
            return Ok(new_name.to_name());
        } else {
            new_name = new_name.generate_next();
        }
    }
}

/// Returns changeset `Merge` such that `Stage<Stage<Stage<Base, Remote>, Local>, Merge>` is a valid tree
fn get_merge_changes<Base, Remote, Local>(
    account: &Account, base: Base, remote_changes: Remote, local_changes: Local,
    base_documents: &HashMap<Uuid, EncryptedDocument>,
    local_document_changes: &HashMap<Uuid, EncryptedDocument>,
    remote_document_changes: &HashMap<Uuid, EncryptedDocument>,
) -> Result<(Vec<SignedFile>, HashMap<Uuid, EncryptedDocument>), CoreError>
where
    Base: Stagable<F = SignedFile>,
    Remote: Stagable<F = SignedFile>,
    Local: Stagable<F = SignedFile>,
{
    let mut merge_document_changes = HashMap::new();
    let base = base.to_lazy();
    let remote = base.stage(remote_changes);
    let local = remote.stage(local_changes);
    let mut merge = local.stage(Vec::new());

    // merge files on an individual basis
    {
        for id in remote_changes.owned_ids() {
            if let Some(local_change) = local_changes.maybe_find(&id) {
                let remote_change = remote_changes.find(&id)?;
                // 3-way merge
                if let Some(base_file) = base.maybe_find(&id) {
                    let parent = *three_way_merge(
                        base_file.parent(),
                        remote_change.parent(),
                        local_change.parent(),
                        remote_change.parent(),
                    );

                    merge.insert(
                        FileMetadata {
                            id,
                            file_type: base_file.file_type(),
                            parent,
                            name: SecretFileName::from_str(
                                three_way_merge(
                                    &base.name(&id, account)?,
                                    &remote.name(&id, account)?,
                                    &local.name(&id, account)?,
                                    &remote.name(&id, account)?,
                                ),
                                &merge.decrypt_key(&id, account)?,
                                &merge.decrypt_key(&parent, account)?,
                            )?,
                            owner: base_file.owner(),
                            is_deleted: remote.calculate_deleted(&id)?
                                | local_change.explicitly_deleted(),
                            document_hmac: three_way_merge(
                                &base_file.document_hmac(),
                                &remote_change.document_hmac(),
                                &local_change.document_hmac(),
                                &None, // overwritten during document merge if local != remote
                            )
                            .cloned(),
                            user_access_keys: base_file.user_access_keys().clone(),
                            folder_access_keys: base_file.folder_access_keys().clone(),
                        }
                        .sign(account)?,
                    );
                }
                // 2-way merge
                else {
                    merge.insert(
                        FileMetadata {
                            id,
                            file_type: remote_change.file_type(),
                            parent: *remote_change.parent(),
                            name: SecretFileName::from_str(
                                &remote.name(&id, account)?,
                                &merge.decrypt_key(&id, account)?,
                                &merge.decrypt_key(remote_change.parent(), account)?,
                            )?,
                            owner: remote_change.owner(),
                            is_deleted: remote.calculate_deleted(&id)?
                                | local_change.explicitly_deleted(),
                            document_hmac: remote_change.document_hmac().cloned(), // overwritten during document merge if local != remote
                            user_access_keys: remote_change.user_access_keys().clone(),
                            folder_access_keys: remote_change.folder_access_keys().clone(),
                        }
                        .sign(account)?,
                    );
                }
            }
        }
    }

    // merge documents
    {
        for (id, remote_document_change) in remote_document_changes {
            // todo: use merged document type
            let local_document_type =
                DocumentType::from_file_name_using_extension(&merge.name(id, account)?);
            match (local_document_changes.get(id), local_document_type) {
                // no local changes -> no merge
                (None, _) => {}
                // text files always merged
                (Some(local_document_change), DocumentType::Text) => {
                    let decrypted_base_document = base_documents
                        .get(id)
                        .map(|document| base.decrypt_document(id, document, account))
                        .map_or(Ok(None), |v| v.map(Some))?
                        .unwrap_or_default();
                    let decrypted_remote_document =
                        remote.decrypt_document(id, remote_document_change, account)?;
                    let decrypted_local_document =
                        local.decrypt_document(id, remote_document_change, account)?;
                    let merged_document = match diffy::merge_bytes(
                        &decrypted_base_document,
                        &decrypted_remote_document,
                        &decrypted_local_document,
                    ) {
                        Ok(without_conflicts) => without_conflicts,
                        Err(with_conflicts) => with_conflicts,
                    };
                    let (new_merged, encrypted_document) =
                        merge.update_document(id, &merged_document, account)?;
                    merge_document_changes.insert(id, encrypted_document);
                    merge = new_merged;
                }
                // non-text files always duplicated
                (Some(local_document_change), DocumentType::Drawing | DocumentType::Other) => {
                    // overwrite existing document (todo: avoid decrypting and re-encrypting document)
                    let decrypted_remote_document =
                        remote.decrypt_document(id, remote_document_change, account)?;
                    let (new_merged, encrypted_document) =
                        merge.update_document(id, &decrypted_remote_document, account)?;
                    merge_document_changes.insert(id, encrypted_document);

                    // create copied document (todo: avoid decrypting and re-encrypting document)
                    let decrypted_local_document =
                        local.decrypt_document(id, remote_document_change, account)?;
                    let existing_document = merge.find(id)?;
                    let (new_merged, copied_document_id) = new_merged.create(
                        existing_document.parent(),
                        &merge.name(id, account)?,
                        existing_document.file_type(),
                        account,
                        &account.public_key(),
                    )?;
                    let (new_merged, encrypted_document) = new_merged.update_document(
                        &copied_document_id,
                        &decrypted_local_document,
                        account,
                    )?;

                    merge = new_merged;
                }
            }
        }
    }

    // merge file trees
    let x = {
        // todo: optimize subroutines by checking only staged things
        let mut this = base.stage(remote_changes).stage(local_changes).to_lazy();
        let mut change = this.unmove_moved_files_in_cycles()?;
        this = this.stage(change).promote_to_local();
        change = this.rename_files_with_path_conflicts(account)?;
        this = this.stage(change).promote_to_local();
        Ok(this)
    };

    todo!()
}

fn get_document<Base, Remote>(
    account: &Account, base: Base, remote_changes: Remote, id: Uuid,
) -> CoreResult<Option<EncryptedDocument>>
where
    Base: Stagable<F = SignedFile>,
    Remote: Stagable<F = SignedFile>,
{
    let remote = base.stage(remote_changes).to_lazy();
    let maybe_hmac = remote.find(&id)?.document_hmac();
    Ok(if let Some(hmac) = maybe_hmac {
        Some(api_service::request(account, GetDocumentRequest { id, hmac: hmac.clone() })?.content)
    } else {
        None
    })
}
