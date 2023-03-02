use std::collections::{HashMap, HashSet};

use lockbook_shared::access_info::UserAccessMode;
use lockbook_shared::account::Account;
use lockbook_shared::api::{
    ChangeDocRequest, GetDocRequest, GetFileIdsRequest, GetUpdatesRequest, GetUpdatesResponse,
    GetUsernameError, GetUsernameRequest, UpsertRequest,
};
use lockbook_shared::core_config::Config;
use lockbook_shared::file::{File, ShareMode};
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{FileDiff, FileType, Owner};
use lockbook_shared::filename::{DocumentType, NameComponents};
use lockbook_shared::signed_file::SignedFile;
use lockbook_shared::staged::{StagedTree, StagedTreeLikeMut};
use lockbook_shared::tree_like::{TreeLike, TreeLikeMut};
use lockbook_shared::work_unit::{ClientWorkUnit, WorkUnit};
use lockbook_shared::{document_repo, symkey, SharedErrorKind, ValidationFailure};

use db_rs::LookupTable;
use itertools::Itertools;
use serde::Serialize;
use uuid::Uuid;

use crate::service::api_service::ApiError;
use crate::{CoreError, CoreState, LbResult, Requester};

#[derive(Debug, Serialize, Clone)]
pub struct WorkCalculated {
    pub work_units: Vec<WorkUnit>,
    pub most_recent_update_from_server: u64,
}

#[derive(Clone)]
pub struct SyncProgress {
    pub total: usize,
    pub progress: usize,
    pub current_work_unit: ClientWorkUnit,
}

enum SyncOperation {
    PullMetadataStart,
    PullMetadataEnd(Vec<File>),
    PushMetadataStart(Vec<File>),
    PushMetadataEnd,
    PullDocumentStart(File),
    PullDocumentEnd,
    PushDocumentStart(File),
    PushDocumentEnd,
}

fn get_work_units(op: &SyncOperation) -> Vec<WorkUnit> {
    let mut work_units: Vec<WorkUnit> = Vec::new();
    match op {
        SyncOperation::PullMetadataEnd(files) => {
            for file in files {
                work_units.push(WorkUnit::ServerChange { metadata: file.clone() });
            }
        }
        SyncOperation::PushMetadataStart(files) => {
            for file in files {
                work_units.push(WorkUnit::LocalChange { metadata: file.clone() });
            }
        }
        SyncOperation::PullMetadataStart
        | SyncOperation::PushMetadataEnd
        | SyncOperation::PullDocumentStart(_)
        | SyncOperation::PullDocumentEnd
        | SyncOperation::PushDocumentStart(_)
        | SyncOperation::PushDocumentEnd => {}
    }
    work_units
}

impl<Client: Requester> CoreState<Client> {
    #[instrument(level = "debug", skip_all, err(Debug))]
    pub(crate) fn calculate_work(&mut self) -> LbResult<WorkCalculated> {
        let mut work_units: Vec<WorkUnit> = Vec::new();
        let mut sync_context = SyncContext {
            dry_run: true,
            account: self.get_account()?.clone(),
            root: self.db.root.data().cloned(),
            last_synced: self.db.last_synced.data().map(|s| *s as u64),
            base: (&self.db.base_metadata).to_staged(Vec::new()),
            local: (&self.db.local_metadata).to_staged(Vec::new()),
            username_by_public_key: &mut self.db.pub_key_lookup,
            client: &self.client,
            config: &self.config,
        };
        let update_as_of = sync_context.sync(&mut |op| work_units.extend(get_work_units(&op)))?;

        Ok(WorkCalculated { work_units, most_recent_update_from_server: update_as_of as u64 })
    }

    #[instrument(level = "debug", skip_all, err(Debug))]
    pub(crate) fn sync<F: Fn(SyncProgress)>(
        &mut self, maybe_update_sync_progress: Option<F>,
    ) -> LbResult<WorkCalculated> {
        let mut work_units: Vec<WorkUnit> = Vec::new();
        let mut sync_progress =
            SyncProgress { total: 0, progress: 0, current_work_unit: ClientWorkUnit::PullMetadata };

        // if sync progress is requested, calculate total work using a dry run
        // this is not the same as the length of work units from work_calculated because e.g.
        // all the metadata pushes are considered one unit of progress, whereas in work calculated
        // each changed file is modeled as a separate work unit
        if maybe_update_sync_progress.is_some() {
            let mut sync_context = SyncContext {
                dry_run: true,
                account: self.get_account()?.clone(),
                root: self.db.root.data().cloned(),
                last_synced: self.db.last_synced.data().map(|s| *s as u64),
                base: (&self.db.base_metadata).to_staged(Vec::new()),
                local: (&self.db.local_metadata).to_staged(Vec::new()),
                username_by_public_key: &mut self.db.pub_key_lookup,
                client: &self.client,
                config: &self.config,
            };
            sync_context.sync(&mut |op| match op {
                SyncOperation::PullMetadataStart
                | SyncOperation::PushMetadataStart(_)
                | SyncOperation::PullDocumentStart(_)
                | SyncOperation::PushDocumentStart(_) => {}
                SyncOperation::PullMetadataEnd(_)
                | SyncOperation::PushMetadataEnd
                | SyncOperation::PullDocumentEnd
                | SyncOperation::PushDocumentEnd => {
                    sync_progress.total += 1;
                }
            })?;
        }
        let mut update_sync_progress = |op| {
            work_units.extend(get_work_units(&op));
            if let Some(ref update_sync_progress) = maybe_update_sync_progress {
                match op {
                    SyncOperation::PullMetadataStart => {
                        sync_progress.current_work_unit = ClientWorkUnit::PullMetadata;
                    }
                    SyncOperation::PushMetadataStart(_) => {
                        sync_progress.current_work_unit = ClientWorkUnit::PushMetadata;
                    }
                    SyncOperation::PullDocumentStart(file) => {
                        sync_progress.current_work_unit = ClientWorkUnit::PullDocument(file.name);
                    }
                    SyncOperation::PushDocumentStart(file) => {
                        sync_progress.current_work_unit = ClientWorkUnit::PushDocument(file.name);
                    }
                    SyncOperation::PullMetadataEnd(_)
                    | SyncOperation::PushMetadataEnd
                    | SyncOperation::PullDocumentEnd
                    | SyncOperation::PushDocumentEnd => {
                        sync_progress.progress += 1;
                    }
                }
                update_sync_progress(sync_progress.clone());
            }
        };

        let mut sync_context = SyncContext {
            dry_run: false,
            account: self.get_account()?.clone(),
            root: self.db.root.data().cloned(),
            last_synced: self.db.last_synced.data().map(|s| *s as u64),
            base: (&mut self.db.base_metadata).to_staged(Vec::new()),
            local: (&mut self.db.local_metadata).to_staged(Vec::new()),
            username_by_public_key: &mut self.db.pub_key_lookup,
            client: &self.client,
            config: &self.config,
        };

        let update_as_of = sync_context.sync(&mut update_sync_progress)?;

        if let Some(root) = sync_context.root {
            self.db.root.insert(root)?;
        }
        if let Some(last_synced) = sync_context.last_synced {
            self.db.last_synced.insert(last_synced as i64)?;
        }
        sync_context.base.to_lazy().promote()?;
        sync_context.local.to_lazy().promote()?;

        Ok(WorkCalculated { work_units, most_recent_update_from_server: update_as_of as u64 })
    }
}

struct SyncContext<'a, Base, Local, Client>
where
    Base: TreeLike<F = SignedFile>,
    Local: TreeLike<F = SignedFile>,
    Client: Requester,
{
    // when dry_run is true, sync skips document downloads/uploads and disk reads/writes
    dry_run: bool,

    // root, last_synced, base metadata, and local metadata have values pre-loaded, are read from
    // and written to here, and are "committed" if we are not doing a dry run
    root: Option<Uuid>,
    last_synced: Option<u64>,
    base: StagedTree<Base, Vec<SignedFile>>,
    local: StagedTree<Local, Vec<SignedFile>>,

    // public key cache is written to, dry run or not
    username_by_public_key: &'a mut LookupTable<Owner, String>,

    // account pre-loaded for convenience
    account: Account,

    // client for network requests and config for disk i/o
    client: &'a Client,
    config: &'a Config,
}

impl<'a, Base, Local, Client> SyncContext<'a, Base, Local, Client>
where
    Base: TreeLike<F = SignedFile>,
    Local: TreeLike<F = SignedFile>,
    Client: Requester,
{
    fn sync<F>(&mut self, report_sync_operation: &mut F) -> LbResult<i64>
    where
        F: FnMut(SyncOperation),
    {
        self.prune()?;
        let update_as_of = self.pull(report_sync_operation)?;
        self.last_synced = Some(update_as_of as u64);
        self.push_metadata(report_sync_operation)?;
        self.push_documents(report_sync_operation)?;
        Ok(update_as_of)
    }

    /// Pulls remote changes and constructs a changeset Merge such that Stage<Stage<Stage<Base, Remote>, Local>, Merge> is valid.
    /// Promotes Base to Stage<Base, Remote> and Local to Stage<Local, Merge>
    fn pull<F>(&mut self, report_sync_operation: &mut F) -> LbResult<i64>
    where
        F: FnMut(SyncOperation),
    {
        // fetch metadata updates
        // todo: use a single fetch of metadata updates for calculating sync progress total and for running the sync
        let (mut remote_changes, update_as_of) = {
            report_sync_operation(SyncOperation::PullMetadataStart);

            let updates = self.get_updates()?;
            let mut remote_changes = updates.file_metadata;
            let update_as_of = updates.as_of_metadata_version;

            remote_changes = self.prune_remote_orphans(remote_changes)?;
            self.populate_public_key_cache(&remote_changes)?;

            let mut remote = self.base.stage(remote_changes).pruned()?.to_lazy();
            report_sync_operation(SyncOperation::PullMetadataEnd(
                remote.resolve_and_finalize_all(
                    &self.account,
                    remote.tree.staged.owned_ids().into_iter(),
                    self.username_by_public_key,
                )?,
            ));
            let (_, remote_changes) = remote.unstage();
            (remote_changes, update_as_of)
        };

        // initialize root if this is the first pull on this device
        if self.root.is_none() {
            let root = remote_changes
                .all_files()?
                .into_iter()
                .find(|f| f.is_root())
                .ok_or(CoreError::RootNonexistent)?;
            self.root = Some(*root.id());
        }

        // fetch document updates and local documents for merge
        let me = Owner(self.account.public_key());
        remote_changes = {
            let mut remote = self.base.stage(remote_changes).to_lazy();
            for id in remote.tree.staged.owned_ids() {
                if remote.calculate_deleted(&id)? {
                    continue;
                }
                let remote_hmac = remote.find(&id)?.document_hmac().cloned();
                let base_hmac = remote
                    .tree
                    .base
                    .maybe_find(&id)
                    .and_then(|f| f.document_hmac())
                    .cloned();
                if base_hmac == remote_hmac {
                    continue;
                }

                if let Some(remote_hmac) = remote_hmac {
                    report_sync_operation(SyncOperation::PullDocumentStart(remote.finalize(
                        &id,
                        &self.account,
                        self.username_by_public_key,
                    )?));

                    if !self.dry_run {
                        let remote_document = self
                            .client
                            .request(&self.account, GetDocRequest { id, hmac: remote_hmac })?
                            .content;
                        document_repo::insert(
                            self.config,
                            &id,
                            Some(&remote_hmac),
                            &remote_document,
                        )?;
                    }

                    report_sync_operation(SyncOperation::PullDocumentEnd);
                }
            }
            let (_, remote_changes) = remote.unstage();
            remote_changes
        };

        // compute merge changes
        let merge_changes = {
            // assemble trees
            let mut base = (&self.base).to_lazy();
            let remote_unlazy = (&self.base).to_staged(&remote_changes);
            let mut remote = remote_unlazy.as_lazy();
            let mut local = (&self.base).to_staged(&self.local).to_lazy();

            // changeset constraints - these evolve as we try to assemble changes and encounter validation failures
            let mut files_to_unmove: HashSet<Uuid> = HashSet::new();
            let mut files_to_unshare: HashSet<Uuid> = HashSet::new();
            let mut links_to_delete: HashSet<Uuid> = HashSet::new();
            let mut rename_increments: HashMap<Uuid, usize> = HashMap::new();
            let mut duplicate_file_ids: HashMap<Uuid, Uuid> = HashMap::new();

            'merge_construction: loop {
                // process just the edits which allow us to check deletions in the result
                let mut deletions = {
                    let mut deletions = remote_unlazy.stage(Vec::new()).to_lazy();

                    // creations
                    let mut deletion_creations = HashSet::new();
                    for id in self.local.owned_ids() {
                        if remote.maybe_find(&id).is_none() && !links_to_delete.contains(&id) {
                            deletion_creations.insert(id);
                        }
                    }
                    'drain_creations: while !deletion_creations.is_empty() {
                        'choose_a_creation: for id in &deletion_creations {
                            // create
                            let id = *id;
                            let local_file = local.find(&id)?.clone();
                            let result = deletions.create_unvalidated(
                                id,
                                symkey::generate_key(),
                                local_file.parent(),
                                &local.name(&id, &self.account)?,
                                local_file.file_type(),
                                &self.account,
                            );
                            match result {
                                Ok(_) => {
                                    deletion_creations.remove(&id);
                                    continue 'drain_creations;
                                }
                                Err(ref err) => match err.kind {
                                    SharedErrorKind::FileParentNonexistent => {
                                        continue 'choose_a_creation;
                                    }
                                    _ => {
                                        result?;
                                    }
                                },
                            }
                        }
                        return Err(CoreError::Unexpected(format!(
                            "sync failed to find a topological order for file creations: {:?}",
                            deletion_creations
                        ))
                        .into());
                    }

                    // moves (creations happen first in case a file is moved into a new folder)
                    for id in self.local.owned_ids() {
                        let local_file = local.find(&id)?.clone();
                        if let Some(base_file) = self.base.maybe_find(&id).cloned() {
                            if !local_file.explicitly_deleted()
                                && local_file.parent() != base_file.parent()
                                && !files_to_unmove.contains(&id)
                            {
                                // move
                                deletions.move_unvalidated(
                                    &id,
                                    local_file.parent(),
                                    &self.account,
                                )?;
                            }
                        }
                    }

                    // deletions (moves happen first in case a file is moved into a deleted folder)
                    for id in self.local.owned_ids() {
                        let local_file = local.find(&id)?.clone();
                        if local_file.explicitly_deleted() {
                            // delete
                            deletions.delete_unvalidated(&id, &self.account)?;
                        }
                    }
                    deletions
                };

                // process all edits, dropping non-deletion edits for files that will be implicitly deleted
                let mut merge = {
                    let mut merge = remote_unlazy.stage(Vec::new()).to_lazy();

                    // creations and edits of created documents
                    let mut creations = HashSet::new();
                    for id in self.local.owned_ids() {
                        if deletions.maybe_find(&id).is_some()
                            && !deletions.calculate_deleted(&id)?
                            && remote.maybe_find(&id).is_none()
                            && !links_to_delete.contains(&id)
                        {
                            creations.insert(id);
                        }
                    }
                    'drain_creations: while !creations.is_empty() {
                        'choose_a_creation: for id in &creations {
                            // create
                            let id = *id;
                            let local_file = local.find(&id)?.clone();
                            let result = merge.create_unvalidated(
                                id,
                                local.decrypt_key(&id, &self.account)?,
                                local_file.parent(),
                                &local.name(&id, &self.account)?,
                                local_file.file_type(),
                                &self.account,
                            );
                            match result {
                                Ok(_) => {
                                    creations.remove(&id);
                                    continue 'drain_creations;
                                }
                                Err(ref err) => match err.kind {
                                    SharedErrorKind::FileParentNonexistent => {
                                        continue 'choose_a_creation;
                                    }
                                    _ => {
                                        result?;
                                    }
                                },
                            }
                        }
                        return Err(CoreError::Unexpected(format!(
                            "sync failed to find a topological order for file creations: {:?}",
                            creations
                        ))
                        .into());
                    }

                    // moves, renames, edits, and shares
                    // creations happen first in case a file is moved into a new folder
                    for id in self.local.owned_ids() {
                        // skip files that are already deleted or will be deleted
                        if deletions.maybe_find(&id).is_none()
                            || deletions.calculate_deleted(&id)?
                            || (remote.maybe_find(&id).is_some()
                                && remote.calculate_deleted(&id)?)
                        {
                            continue;
                        }

                        let local_file = local.find(&id)?.clone();
                        let local_name = local.name(&id, &self.account)?;
                        let maybe_base_file = base.maybe_find(&id).cloned();
                        let maybe_remote_file = remote.maybe_find(&id).cloned();
                        if let Some(ref base_file) = maybe_base_file {
                            let base_name = base.name(&id, &self.account)?;
                            let remote_file = remote.find(&id)?.clone();
                            let remote_name = remote.name(&id, &self.account)?;

                            // move
                            if local_file.parent() != base_file.parent()
                                && remote_file.parent() == base_file.parent()
                                && !files_to_unmove.contains(&id)
                            {
                                merge.move_unvalidated(&id, local_file.parent(), &self.account)?;
                            }

                            // rename
                            if local_name != base_name && remote_name == base_name {
                                merge.rename_unvalidated(&id, &local_name, &self.account)?;
                            }
                        }

                        // share
                        let mut remote_keys = HashMap::new();
                        if let Some(ref remote_file) = maybe_remote_file {
                            for key in remote_file.user_access_keys() {
                                remote_keys.insert(
                                    (Owner(key.encrypted_by), Owner(key.encrypted_for)),
                                    (key.mode, key.deleted),
                                );
                            }
                        }
                        for key in local_file.user_access_keys() {
                            let (by, for_) = (Owner(key.encrypted_by), Owner(key.encrypted_for));
                            if let Some(&(remote_mode, remote_deleted)) =
                                remote_keys.get(&(by, for_))
                            {
                                // upgrade share
                                if key.mode > remote_mode || !key.deleted && remote_deleted {
                                    let mode = match key.mode {
                                        UserAccessMode::Read => ShareMode::Read,
                                        UserAccessMode::Write => ShareMode::Write,
                                        UserAccessMode::Owner => continue,
                                    };
                                    merge.add_share_unvalidated(id, for_, mode, &self.account)?;
                                }
                                // delete share
                                if key.deleted && !remote_deleted {
                                    merge.delete_share_unvalidated(
                                        &id,
                                        Some(for_.0),
                                        &self.account,
                                    )?;
                                }
                            } else {
                                // add share
                                let mode = match key.mode {
                                    UserAccessMode::Read => ShareMode::Read,
                                    UserAccessMode::Write => ShareMode::Write,
                                    UserAccessMode::Owner => continue,
                                };
                                merge.add_share_unvalidated(id, for_, mode, &self.account)?;
                            }
                        }

                        // share deletion due to conflicts
                        if files_to_unshare.contains(&id) {
                            merge.delete_share_unvalidated(&id, None, &self.account)?;
                        }

                        // rename due to path conflict
                        if let Some(&rename_increment) = rename_increments.get(&id) {
                            let name = NameComponents::from(&local_name)
                                .generate_incremented(rename_increment)
                                .to_name();
                            merge.rename_unvalidated(&id, &name, &self.account)?;
                        }

                        // edit
                        let base_hmac = maybe_base_file.and_then(|f| f.document_hmac().cloned());
                        let remote_hmac =
                            maybe_remote_file.and_then(|f| f.document_hmac().cloned());
                        let local_hmac = local_file.document_hmac().cloned();
                        if merge.access_mode(me, &id)? >= Some(UserAccessMode::Write)
                            && local_hmac != base_hmac
                        {
                            if remote_hmac != base_hmac && remote_hmac != local_hmac {
                                // merge
                                let merge_name = merge.name(&id, &self.account)?;
                                let document_type =
                                    DocumentType::from_file_name_using_extension(&merge_name);
                                let base_document = if base_hmac.is_some() {
                                    base.read_document(self.config, &id, &self.account)?
                                } else {
                                    Vec::new()
                                };
                                let remote_document = if remote_hmac.is_some() {
                                    remote.read_document(self.config, &id, &self.account)?
                                } else {
                                    Vec::new()
                                };
                                let local_document = if local_hmac.is_some() {
                                    local.read_document(self.config, &id, &self.account)?
                                } else {
                                    Vec::new()
                                };
                                match document_type {
                                    DocumentType::Text => {
                                        // 3-way merge
                                        let merged_document = match diffy::merge_bytes(
                                            &base_document,
                                            &remote_document,
                                            &local_document,
                                        ) {
                                            Ok(without_conflicts) => without_conflicts,
                                            Err(with_conflicts) => with_conflicts,
                                        };
                                        let encrypted_document = merge
                                            .update_document_unvalidated(
                                                &id,
                                                &merged_document,
                                                &self.account,
                                            )?;
                                        if !self.dry_run {
                                            let hmac = merge.find(&id)?.document_hmac();
                                            document_repo::insert(
                                                self.config,
                                                &id,
                                                hmac,
                                                &encrypted_document,
                                            )?;
                                        }
                                    }
                                    DocumentType::Drawing | DocumentType::Other => {
                                        // duplicate file
                                        let merge_parent = *merge.find(&id)?.parent();
                                        let duplicate_id = if let Some(&duplicate_id) =
                                            duplicate_file_ids.get(&id)
                                        {
                                            duplicate_id
                                        } else {
                                            let duplicate_id = Uuid::new_v4();
                                            duplicate_file_ids.insert(id, duplicate_id);
                                            rename_increments.insert(duplicate_id, 1);
                                            duplicate_id
                                        };

                                        let mut merge_name = merge_name;
                                        merge_name = NameComponents::from(&merge_name)
                                            .generate_incremented(
                                                rename_increments
                                                    .get(&duplicate_id)
                                                    .copied()
                                                    .unwrap_or_default(),
                                            )
                                            .to_name();

                                        merge.create_unvalidated(
                                            duplicate_id,
                                            symkey::generate_key(),
                                            &merge_parent,
                                            &merge_name,
                                            FileType::Document,
                                            &self.account,
                                        )?;
                                        let encrypted_document = merge
                                            .update_document_unvalidated(
                                                &duplicate_id,
                                                &local_document,
                                                &self.account,
                                            )?;
                                        if !self.dry_run {
                                            let duplicate_hmac =
                                                merge.find(&duplicate_id)?.document_hmac();
                                            document_repo::insert(
                                                self.config,
                                                &duplicate_id,
                                                duplicate_hmac,
                                                &encrypted_document,
                                            )?;
                                        }
                                    }
                                }
                            } else {
                                // overwrite (todo: avoid reading/decrypting/encrypting document)
                                let document =
                                    local.read_document(self.config, &id, &self.account)?;
                                merge.update_document_unvalidated(&id, &document, &self.account)?;
                            }
                        }
                    }

                    // deletes
                    // moves happen first in case a file is moved into a deleted folder
                    for id in self.local.owned_ids() {
                        if self.base.maybe_find(&id).is_some()
                            && deletions.calculate_deleted(&id)?
                            && !merge.calculate_deleted(&id)?
                        {
                            // delete
                            merge.delete_unvalidated(&id, &self.account)?;
                        }
                    }
                    for &id in &links_to_delete {
                        // delete
                        if merge.maybe_find(&id).is_some() && !merge.calculate_deleted(&id)? {
                            merge.delete_unvalidated(&id, &self.account)?;
                        }
                    }

                    merge
                };

                // validate; handle failures by introducing changeset constraints
                for link in merge.owned_ids() {
                    if !merge.calculate_deleted(&link)? {
                        if let FileType::Link { target } = merge.find(&link)?.file_type() {
                            if merge.maybe_find(&target).is_some()
                                && merge.calculate_deleted(&target)?
                            {
                                // delete links to deleted files
                                if links_to_delete.insert(link) {
                                    continue 'merge_construction;
                                } else {
                                    return Err(CoreError::Unexpected(format!(
                                        "sync failed to resolve broken link (deletion): {:?}",
                                        link
                                    ))
                                    .into());
                                }
                            }
                        }
                    }
                }

                let validate_result = merge.validate(me);
                match validate_result {
                    // merge changeset is valid
                    Ok(_) => {
                        let (_, merge_changes) = merge.unstage();
                        break merge_changes;
                    }
                    Err(ref err) => match err.kind {
                        SharedErrorKind::ValidationFailure(ref vf) => match vf {
                            // merge changeset has resolvable validation errors and needs modification
                            ValidationFailure::Cycle(ids) => {
                                // revert all local moves in the cycle
                                let mut progress = false;
                                for &id in ids {
                                    if self.local.maybe_find(&id).is_some()
                                        && files_to_unmove.insert(id)
                                    {
                                        progress = true;
                                    }
                                }
                                if !progress {
                                    return Err(CoreError::Unexpected(format!(
                                        "sync failed to resolve cycle: {:?}",
                                        ids
                                    ))
                                    .into());
                                }
                            }
                            ValidationFailure::PathConflict(ids) => {
                                // pick one local id and generate a non-conflicting filename
                                let mut progress = false;
                                for &id in ids {
                                    if duplicate_file_ids.values().contains(&id) {
                                        *rename_increments.entry(id).or_insert(0) += 1;
                                        progress = true;
                                        break;
                                    }
                                }
                                if !progress {
                                    for &id in ids {
                                        if self.local.maybe_find(&id).is_some() {
                                            *rename_increments.entry(id).or_insert(0) += 1;
                                            progress = true;
                                            break;
                                        }
                                    }
                                }
                                if !progress {
                                    return Err(CoreError::Unexpected(format!(
                                        "sync failed to resolve path conflict: {:?}",
                                        ids
                                    ))
                                    .into());
                                }
                            }
                            ValidationFailure::SharedLink { link, shared_ancestor } => {
                                // if ancestor is newly shared, delete share, otherwise delete link
                                let mut progress = false;
                                if let Some(base_shared_ancestor) = base.maybe_find(shared_ancestor)
                                {
                                    if !base_shared_ancestor.is_shared()
                                        && files_to_unshare.insert(*shared_ancestor)
                                    {
                                        progress = true;
                                    }
                                }
                                if !progress && links_to_delete.insert(*link) {
                                    progress = true;
                                }
                                if !progress {
                                    return Err(CoreError::Unexpected(format!(
                                    "sync failed to resolve shared link: link: {:?}, shared_ancestor: {:?}",
                                    link, shared_ancestor
                                )).into());
                                }
                            }
                            ValidationFailure::DuplicateLink { target } => {
                                // delete local link with this target
                                let mut progress = false;
                                if let Some(link) = local.link(target)? {
                                    if links_to_delete.insert(link) {
                                        progress = true;
                                    }
                                }
                                if !progress {
                                    return Err(CoreError::Unexpected(format!(
                                        "sync failed to resolve duplicate link: target: {:?}",
                                        target
                                    ))
                                    .into());
                                }
                            }
                            ValidationFailure::BrokenLink(link) => {
                                // delete local link with this target
                                if !links_to_delete.insert(*link) {
                                    return Err(CoreError::Unexpected(format!(
                                        "sync failed to resolve broken link: {:?}",
                                        link
                                    ))
                                    .into());
                                }
                            }
                            ValidationFailure::OwnedLink(link) => {
                                // if target is newly owned, unmove target, otherwise delete link
                                let mut progress = false;
                                if let Some(remote_link) = remote.maybe_find(link) {
                                    if let FileType::Link { target } = remote_link.file_type() {
                                        let remote_target = remote.find(&target)?;
                                        if remote_target.owner() != me
                                            && files_to_unmove.insert(target)
                                        {
                                            progress = true;
                                        }
                                    }
                                }
                                if !progress && links_to_delete.insert(*link) {
                                    progress = true;
                                }
                                if !progress {
                                    return Err(CoreError::Unexpected(format!(
                                        "sync failed to resolve owned link: {:?}",
                                        link
                                    ))
                                    .into());
                                }
                            }
                            // merge changeset has unexpected validation errors
                            ValidationFailure::Orphan(_)
                            | ValidationFailure::NonFolderWithChildren(_)
                            | ValidationFailure::FileWithDifferentOwnerParent(_)
                            | ValidationFailure::NonDecryptableFileName(_) => {
                                validate_result?;
                            }
                        },
                        // merge changeset has unexpected errors
                        _ => {
                            validate_result?;
                        }
                    },
                }
            }
        };

        // base = remote; local = merge
        (&mut self.base)
            .to_staged(remote_changes)
            .to_lazy()
            .promote()?;
        self.local.clear()?;
        (&mut self.local)
            .to_staged(merge_changes)
            .to_lazy()
            .promote()?;
        self.cleanup_local_metadata()?;

        Ok(update_as_of as i64)
    }

    pub(crate) fn get_updates(&self) -> LbResult<GetUpdatesResponse> {
        let last_synced = self.last_synced.unwrap_or_default();
        let remote_changes = self
            .client
            .request(&self.account, GetUpdatesRequest { since_metadata_version: last_synced })?;
        Ok(remote_changes)
    }

    pub(crate) fn prune_remote_orphans(
        &mut self, remote_changes: Vec<SignedFile>,
    ) -> LbResult<Vec<SignedFile>> {
        let me = Owner(self.account.public_key());
        let remote = self.base.stage(remote_changes).to_lazy();
        let mut result = Vec::new();
        for id in remote.tree.staged.owned_ids() {
            let meta = remote.find(&id)?;
            if remote.maybe_find_parent(meta).is_some()
                || meta
                    .user_access_keys()
                    .iter()
                    .any(|k| k.encrypted_for == me.0)
            {
                result.push(remote.find(&id)?.clone()); // todo: don't clone
            }
        }
        Ok(result)
    }

    fn prune(&mut self) -> LbResult<()> {
        let mut local = self.base.stage(&self.local).to_lazy();
        let base_ids = local.tree.base.owned_ids();
        let server_ids = self
            .client
            .request(&self.account, GetFileIdsRequest {})?
            .ids;

        let mut prunable_ids = base_ids;
        prunable_ids.retain(|id| !server_ids.contains(id));
        for id in prunable_ids.clone() {
            prunable_ids.extend(local.descendants(&id)?.into_iter());
        }
        if !self.dry_run {
            for id in &prunable_ids {
                if let Some(base_file) = local.tree.base.maybe_find(id) {
                    document_repo::delete(self.config, id, base_file.document_hmac())?;
                }
                if let Some(local_file) = local.maybe_find(id) {
                    document_repo::delete(self.config, id, local_file.document_hmac())?;
                }
            }
        }

        let mut base_staged = (&mut self.base).to_lazy().stage(None);
        base_staged.tree.removed = prunable_ids.clone();
        base_staged.promote()?;

        let mut local_staged = (&mut self.local).to_lazy().stage(None);
        local_staged.tree.removed = prunable_ids;
        local_staged.promote()?;

        Ok(())
    }

    /// Updates remote and base metadata to local.
    #[instrument(level = "debug", skip_all, err(Debug))]
    fn push_metadata<F>(&mut self, report_sync_operation: &mut F) -> LbResult<()>
    where
        F: FnMut(SyncOperation),
    {
        // remote = local
        let mut local_changes_no_digests = Vec::new();
        let mut updates = Vec::new();
        let mut local = self.base.stage(&self.local).to_lazy();

        for id in local.tree.staged.owned_ids() {
            let mut local_change = local.tree.staged.find(&id)?.timestamped_value.value.clone();
            let maybe_base_file = local.tree.base.maybe_find(&id);

            // change everything but document hmac and re-sign
            local_change.document_hmac =
                maybe_base_file.and_then(|f| f.timestamped_value.value.document_hmac);
            let local_change = local_change.sign(&self.account)?;

            local_changes_no_digests.push(local_change.clone());
            let file_diff = FileDiff { old: maybe_base_file.cloned(), new: local_change };
            updates.push(file_diff);
        }

        report_sync_operation(SyncOperation::PushMetadataStart(local.resolve_and_finalize_all(
            &self.account,
            local.tree.staged.owned_ids().into_iter(),
            self.username_by_public_key,
        )?));

        if !self.dry_run && !updates.is_empty() {
            self.client
                .request(&self.account, UpsertRequest { updates })?;
        }

        report_sync_operation(SyncOperation::PushMetadataEnd);

        // base = local
        (&mut self.base)
            .to_lazy()
            .stage(local_changes_no_digests)
            .promote()?;
        self.cleanup_local_metadata()?;

        Ok(())
    }

    /// Updates remote and base files to local. Assumes metadata is already pushed for all new files.
    #[instrument(level = "debug", skip_all, err(Debug))]
    fn push_documents<F>(&mut self, report_sync_operation: &mut F) -> LbResult<()>
    where
        F: FnMut(SyncOperation),
    {
        let mut local = self.base.stage(&self.local).to_lazy();

        let mut local_changes_digests_only = Vec::new();
        for id in local.tree.staged.owned_ids() {
            let base_file = local.tree.base.find(&id)?.clone();

            // change only document hmac and re-sign
            let mut local_change = base_file.timestamped_value.value.clone();
            local_change.document_hmac = local.find(&id)?.timestamped_value.value.document_hmac;

            if base_file.document_hmac() == local_change.document_hmac()
                || local_change.document_hmac.is_none()
            {
                continue;
            }

            let local_change = local_change.sign(&self.account)?;

            report_sync_operation(SyncOperation::PushDocumentStart(local.finalize(
                &id,
                &self.account,
                self.username_by_public_key,
            )?));

            if !self.dry_run {
                let local_document_change =
                    document_repo::get(self.config, &id, local_change.document_hmac())?;

                // base = local (document)
                // todo: remove?
                document_repo::insert(
                    self.config,
                    &id,
                    local_change.document_hmac(),
                    &local_document_change,
                )?;

                // remote = local
                self.client.request(
                    &self.account,
                    ChangeDocRequest {
                        diff: FileDiff { old: Some(base_file), new: local_change.clone() },
                        new_content: local_document_change,
                    },
                )?;
            }

            report_sync_operation(SyncOperation::PushDocumentEnd);

            local_changes_digests_only.push(local_change);
        }

        // base = local (metadata)
        (&mut self.base)
            .to_lazy()
            .stage(local_changes_digests_only)
            .promote()?;
        self.cleanup_local_metadata()?;

        Ok(())
    }

    fn populate_public_key_cache(&mut self, files: &[SignedFile]) -> LbResult<()> {
        let mut all_owners = HashSet::new();
        for file in files {
            for user_access_key in file.user_access_keys() {
                all_owners.insert(Owner(user_access_key.encrypted_by));
                all_owners.insert(Owner(user_access_key.encrypted_for));
            }
        }

        for owner in all_owners {
            if !self.username_by_public_key.data().contains_key(&owner) {
                let username_result = self
                    .client
                    .request(&self.account, GetUsernameRequest { key: owner.0 });
                let username = match username_result {
                    Err(ApiError::Endpoint(GetUsernameError::UserNotFound)) => {
                        "<unknown>".to_string()
                    }
                    _ => username_result?.username.clone(),
                };
                self.username_by_public_key
                    .insert(owner, username.clone())?;
            }
        }

        Ok(())
    }

    // todo: check only necessary ids
    fn cleanup_local_metadata(&mut self) -> LbResult<()> {
        self.base.stage(&mut self.local).prune()?;
        Ok(())
    }
}
