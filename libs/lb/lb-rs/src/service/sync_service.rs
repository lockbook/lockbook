use std::collections::{HashMap, HashSet};

use lockbook_shared::access_info::UserAccessMode;
use lockbook_shared::account::Account;
use lockbook_shared::api::{
    ChangeDocRequest, GetDocRequest, GetFileIdsRequest, GetUpdatesRequest, GetUsernameError,
    GetUsernameRequest, UpsertRequest,
};
use lockbook_shared::document_repo::DocumentService;
use lockbook_shared::file::ShareMode;
use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::{FileDiff, FileType, Owner};
use lockbook_shared::filename::{DocumentType, NameComponents};
use lockbook_shared::signed_file::SignedFile;
use lockbook_shared::staged::StagedTreeLikeMut;
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::work_unit::{ClientWorkUnit, WorkUnit};
use lockbook_shared::{symkey, SharedErrorKind, ValidationFailure};

use serde::Serialize;
use uuid::Uuid;

use crate::service::api_service::ApiError;
use crate::{CoreError, CoreLib, CoreState, LbResult, Requester};

#[derive(Debug, Serialize, Clone)]
pub struct WorkCalculated {
    pub work_units: Vec<WorkUnit>,
    pub latest_server_ts: u64,
}

#[derive(Clone)]
pub struct SyncProgress {
    pub total: usize,
    pub progress: usize,
    pub current_work_unit: ClientWorkUnit,
}

pub struct SyncContext<Client: Requester, Docs: DocumentService> {
    core: CoreLib<Client, Docs>,
    client: Client,
    docs: Docs,

    account: Account,
    last_synced: u64,
    remote_changes: Vec<SignedFile>,
    update_as_of: u64,
    pushed_metas: Vec<FileDiff<SignedFile>>,
    pushed_docs: Vec<FileDiff<SignedFile>>,
}

impl<Client: Requester, Docs: DocumentService> SyncContext<Client, Docs> {
    pub fn sync(core: &CoreLib<Client, Docs>) -> LbResult<WorkCalculated> {
        let mut context = SyncContext::setup(core)?;

        let sync_result = context
            .fetch_meta()
            .and_then(|_| context.fetch_docs())
            .and_then(|_| context.merge())
            .and_then(|_| context.push_meta())
            .and_then(|_| context.push_docs())
            .and_then(|_| context.commit_last_synced());

        let cleanup = context.must_cleanup();

        sync_result?;
        cleanup?;

        Ok(context.work_calculated())
    }

    fn setup(core: &CoreLib<Client, Docs>) -> LbResult<Self> {
        let mut inner = core.inner.lock()?;
        let core = core.clone();

        inner.syncing = true;
        let client = inner.client.clone();
        let account = inner.get_account()?.clone();
        let last_synced = inner.db.last_synced.get().copied().unwrap_or_default() as u64;
        let docs = inner.docs.clone();

        Ok(Self {
            core,
            client,
            docs,
            account,
            last_synced,

            update_as_of: Default::default(),
            remote_changes: Default::default(),
            pushed_docs: Default::default(),
            pushed_metas: Default::default(),
        })
    }

    fn fetch_meta(&mut self) -> LbResult<()> {
        let updates = self.client.request(
            &self.account,
            GetUpdatesRequest { since_metadata_version: self.last_synced },
        )?;

        self.core.in_tx(|tx| {
            let (mut remote_changes, update_as_of) = {
                let mut remote_changes = updates.file_metadata;
                let update_as_of = updates.as_of_metadata_version;

                remote_changes = tx.prune_remote_orphans(remote_changes)?;
                tx.populate_public_key_cache(&remote_changes)?;

                let remote = (&tx.db.base_metadata)
                    .stage(remote_changes)
                    .pruned()?
                    .to_lazy();

                let (_, remote_changes) = remote.unstage();
                (remote_changes, update_as_of)
            };

            // initialize root if this is the first pull on this device
            if tx.db.root.get().is_none() {
                let root = remote_changes
                    .all_files()?
                    .into_iter()
                    .find(|f| f.is_root())
                    .ok_or(CoreError::RootNonexistent)?;
                tx.db.root.insert(*root.id())?;
            }

            self.remote_changes = remote_changes;
            self.update_as_of = update_as_of;

            Ok(())
        })?;

        Ok(())
    }

    fn fetch_docs(&mut self) -> LbResult<()> {
        let mut docs_to_pull = vec![];

        self.core.in_tx(|tx| {
            let mut remote = (&tx.db.base_metadata).stage(&self.remote_changes).to_lazy(); // this used to be owned remote changes
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
                    docs_to_pull.push((id, remote_hmac));
                }
            }
            Ok(())
        })?;

        for (id, hmac) in docs_to_pull {
            let remote_document = self
                .client
                .request(&self.account, GetDocRequest { id, hmac })?;
            self.docs
                .insert(&id, Some(&hmac), &remote_document.content)?;
        }

        Ok(())
    }

    fn merge(&self) -> LbResult<()> {
        self.core.in_tx(|tx| tx.merge(&self.remote_changes))
    }

    /// Updates remote and base metadata to local.
    fn push_meta(&mut self) -> LbResult<()> {
        let mut updates = vec![];
        let mut local_changes_no_digests = Vec::new();

        self.core.in_tx(|tx| {
            // remote = local
            let mut local = tx.db.base_metadata.stage(&tx.db.local_metadata).to_lazy();

            for id in local.tree.staged.owned_ids() {
                let mut local_change = local.tree.staged.find(&id)?.timestamped_value.value.clone();
                let maybe_base_file = local.tree.base.maybe_find(&id);

                // change everything but document hmac and re-sign
                local_change.document_hmac =
                    maybe_base_file.and_then(|f| f.timestamped_value.value.document_hmac);
                let local_change = local_change.sign(tx.get_account()?)?;

                local_changes_no_digests.push(local_change.clone());
                let file_diff = FileDiff { old: maybe_base_file.cloned(), new: local_change };
                updates.push(file_diff);
            }

            Ok(())
        })?;

        if !updates.is_empty() {
            self.client
                .request(&self.account, UpsertRequest { updates: updates.clone() })?;
            self.pushed_metas = updates;
        }

        self.core.in_tx(|tx| {
            // base = local
            (&mut tx.db.base_metadata)
                .to_lazy()
                .stage(local_changes_no_digests)
                .promote()?;
            tx.cleanup_local_metadata()?;

            Ok(())
        })?;

        Ok(())
    }

    /// Updates remote and base files to local. Assumes metadata is already pushed for all new files.
    fn push_docs(&mut self) -> LbResult<()> {
        let mut updates = vec![];
        let mut local_changes_digests_only = vec![];

        self.core.in_tx(|tx| {
            let mut local = tx.db.base_metadata.stage(&tx.db.local_metadata).to_lazy();

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

                let local_change = local_change.sign(tx.get_account()?)?;

                updates.push(FileDiff { old: Some(base_file), new: local_change.clone() });
                local_changes_digests_only.push(local_change);
            }
            Ok(())
        })?;

        for diff in updates.clone() {
            let id = diff.new.id();
            let hmac = diff.new.document_hmac();

            let local_document_change = self.docs.get(&id, hmac)?;

            // remote = local
            self.client.request(
                &self.account,
                ChangeDocRequest { diff, new_content: local_document_change },
            )?;
        }

        self.pushed_docs = updates;

        self.core.in_tx(|tx| {
            // base = local (metadata)
            (&mut tx.db.base_metadata)
                .to_lazy()
                .stage(local_changes_digests_only)
                .promote()?;
            tx.cleanup_local_metadata()?;
            Ok(())
        })?;

        Ok(())
    }

    fn commit_last_synced(&self) -> LbResult<()> {
        self.core.in_tx(|tx| {
            tx.db.last_synced.insert(self.last_synced as i64)?;
            Ok(())
        })
    }
    fn must_cleanup(&self) -> LbResult<()> {
        self.core.in_tx(|tx| {
            tx.syncing = false;
            tx.cleanup()
        })?;

        Ok(())
    }

    fn work_calculated(&self) -> WorkCalculated {
        let mut local = HashSet::new();
        let mut server = HashSet::new();
        let mut work_units = vec![];

        for meta in &self.pushed_metas {
            local.insert(meta.new.id());
        }

        for meta in &self.pushed_docs {
            local.insert(meta.new.id());
        }

        for meta in &self.remote_changes {
            server.insert(meta.id());
        }

        for id in local {
            work_units.push(WorkUnit::LocalChange(*id));
        }

        for id in server {
            work_units.push(WorkUnit::ServerChange(*id));
        }

        WorkCalculated { work_units, latest_server_ts: self.update_as_of }
    }
}

impl<Client: Requester, Docs: DocumentService> CoreState<Client, Docs> {
    #[instrument(level = "debug", skip_all, err(Debug))]
    pub(crate) fn calculate_work(&mut self) -> LbResult<WorkCalculated> {
        self.prune()?;

        let locally_dirty = self
            .db
            .local_metadata
            .get()
            .keys()
            .copied()
            .map(WorkUnit::LocalChange);

        let last_synced = self.db.last_synced.get().copied().unwrap_or_default() as u64;
        let remote_changes = self.client.request(
            self.get_account()?,
            GetUpdatesRequest { since_metadata_version: last_synced },
        )?;
        let remote_dirty = remote_changes
            .file_metadata
            .into_iter()
            .map(|f| *f.id())
            .map(WorkUnit::ServerChange);

        let mut work_units: Vec<WorkUnit> = Vec::new();
        work_units.extend(locally_dirty.chain(remote_dirty));
        Ok(WorkCalculated { work_units, latest_server_ts: remote_changes.as_of_metadata_version })
    }

    /// Pulls remote changes and constructs a changeset Merge such that Stage<Stage<Stage<Base, Remote>, Local>, Merge> is valid.
    /// Promotes Base to Stage<Base, Remote> and Local to Stage<Local, Merge>
    fn merge(&mut self, remote_changes: &Vec<SignedFile>) -> LbResult<()> {
        // fetch document updates and local documents for merge
        let me = Owner(self.get_public_key()?);

        // compute merge changes
        let merge_changes = {
            // assemble trees
            let mut base = (&self.db.base_metadata).to_lazy();
            let remote_unlazy = (&self.db.base_metadata).to_staged(remote_changes);
            let mut remote = remote_unlazy.as_lazy();
            let mut local = (&self.db.base_metadata)
                .to_staged(&self.db.local_metadata)
                .to_lazy();

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
                    for id in self.db.local_metadata.owned_ids() {
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
                                &local.name(&id, self.get_account()?)?,
                                local_file.file_type(),
                                self.get_account()?,
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
                    for id in self.db.local_metadata.owned_ids() {
                        let local_file = local.find(&id)?.clone();
                        if let Some(base_file) = self.db.base_metadata.maybe_find(&id).cloned() {
                            if !local_file.explicitly_deleted()
                                && local_file.parent() != base_file.parent()
                                && !files_to_unmove.contains(&id)
                            {
                                // move
                                deletions.move_unvalidated(
                                    &id,
                                    local_file.parent(),
                                    self.get_account()?,
                                )?;
                            }
                        }
                    }

                    // deletions (moves happen first in case a file is moved into a deleted folder)
                    for id in self.db.local_metadata.owned_ids() {
                        let local_file = local.find(&id)?.clone();
                        if local_file.explicitly_deleted() {
                            // delete
                            deletions.delete_unvalidated(&id, self.get_account()?)?;
                        }
                    }
                    deletions
                };

                // process all edits, dropping non-deletion edits for files that will be implicitly deleted
                let mut merge = {
                    let mut merge = remote_unlazy.stage(Vec::new()).to_lazy();

                    // creations and edits of created documents
                    let mut creations = HashSet::new();
                    for id in self.db.local_metadata.owned_ids() {
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
                                local.decrypt_key(&id, self.get_account()?)?,
                                local_file.parent(),
                                &local.name(&id, self.get_account()?)?,
                                local_file.file_type(),
                                self.get_account()?,
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
                    for id in self.db.local_metadata.owned_ids() {
                        // skip files that are already deleted or will be deleted
                        if deletions.maybe_find(&id).is_none()
                            || deletions.calculate_deleted(&id)?
                            || (remote.maybe_find(&id).is_some()
                                && remote.calculate_deleted(&id)?)
                        {
                            continue;
                        }

                        let local_file = local.find(&id)?.clone();
                        let local_name = local.name(&id, self.get_account()?)?;
                        let maybe_base_file = base.maybe_find(&id).cloned();
                        let maybe_remote_file = remote.maybe_find(&id).cloned();
                        if let Some(ref base_file) = maybe_base_file {
                            let base_name = base.name(&id, self.get_account()?)?;
                            let remote_file = remote.find(&id)?.clone();
                            let remote_name = remote.name(&id, self.get_account()?)?;

                            // move
                            if local_file.parent() != base_file.parent()
                                && remote_file.parent() == base_file.parent()
                                && !files_to_unmove.contains(&id)
                            {
                                merge.move_unvalidated(
                                    &id,
                                    local_file.parent(),
                                    self.get_account()?,
                                )?;
                            }

                            // rename
                            if local_name != base_name && remote_name == base_name {
                                merge.rename_unvalidated(&id, &local_name, self.get_account()?)?;
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
                                    merge.add_share_unvalidated(
                                        id,
                                        for_,
                                        mode,
                                        self.get_account()?,
                                    )?;
                                }
                                // delete share
                                if key.deleted && !remote_deleted {
                                    merge.delete_share_unvalidated(
                                        &id,
                                        Some(for_.0),
                                        self.get_account()?,
                                    )?;
                                }
                            } else {
                                // add share
                                let mode = match key.mode {
                                    UserAccessMode::Read => ShareMode::Read,
                                    UserAccessMode::Write => ShareMode::Write,
                                    UserAccessMode::Owner => continue,
                                };
                                merge.add_share_unvalidated(id, for_, mode, self.get_account()?)?;
                            }
                        }

                        // share deletion due to conflicts
                        if files_to_unshare.contains(&id) {
                            merge.delete_share_unvalidated(&id, None, self.get_account()?)?;
                        }

                        // rename due to path conflict
                        if let Some(&rename_increment) = rename_increments.get(&id) {
                            let name = NameComponents::from(&local_name)
                                .generate_incremented(rename_increment)
                                .to_name();
                            merge.rename_unvalidated(&id, &name, self.get_account()?)?;
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
                                let merge_name = merge.name(&id, self.get_account()?)?;
                                let document_type =
                                    DocumentType::from_file_name_using_extension(&merge_name);
                                let base_document = if base_hmac.is_some() {
                                    base.read_document(&self.docs, &id, self.get_account()?)?
                                } else {
                                    Vec::new()
                                };
                                let remote_document = if remote_hmac.is_some() {
                                    remote.read_document(&self.docs, &id, self.get_account()?)?
                                } else {
                                    Vec::new()
                                };
                                let local_document = if local_hmac.is_some() {
                                    local.read_document(&self.docs, &id, self.get_account()?)?
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
                                                self.get_account()?,
                                            )?;
                                        let hmac = merge.find(&id)?.document_hmac();
                                        self.docs.insert(&id, hmac, &encrypted_document)?;
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
                                            self.get_account()?,
                                        )?;
                                        let encrypted_document = merge
                                            .update_document_unvalidated(
                                                &duplicate_id,
                                                &local_document,
                                                self.get_account()?,
                                            )?;
                                        let duplicate_hmac =
                                            merge.find(&duplicate_id)?.document_hmac();
                                        self.docs.insert(
                                            &duplicate_id,
                                            duplicate_hmac,
                                            &encrypted_document,
                                        )?;
                                    }
                                }
                            } else {
                                // overwrite (todo: avoid reading/decrypting/encrypting document)
                                let document =
                                    local.read_document(&self.docs, &id, self.get_account()?)?;
                                merge.update_document_unvalidated(
                                    &id,
                                    &document,
                                    self.get_account()?,
                                )?;
                            }
                        }
                    }

                    // deletes
                    // moves happen first in case a file is moved into a deleted folder
                    for id in self.db.local_metadata.owned_ids() {
                        if self.db.base_metadata.maybe_find(&id).is_some()
                            && deletions.calculate_deleted(&id)?
                            && !merge.calculate_deleted(&id)?
                        {
                            // delete
                            merge.delete_unvalidated(&id, self.get_account()?)?;
                        }
                    }
                    for &id in &links_to_delete {
                        // delete
                        if merge.maybe_find(&id).is_some() && !merge.calculate_deleted(&id)? {
                            merge.delete_unvalidated(&id, self.get_account()?)?;
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
                                    if self.db.local_metadata.maybe_find(&id).is_some()
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
                                    if duplicate_file_ids.values().any(|&dup| dup == id) {
                                        *rename_increments.entry(id).or_insert(0) += 1;
                                        progress = true;
                                        break;
                                    }
                                }
                                if !progress {
                                    for &id in ids {
                                        if self.db.local_metadata.maybe_find(&id).is_some() {
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
                                if let Some(link) = local.linked_by(target)? {
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
                            | ValidationFailure::FileNameTooLong(_)
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
        (&mut self.db.base_metadata)
            .to_staged(remote_changes.clone())
            .to_lazy()
            .promote()?;
        self.db.local_metadata.clear()?;
        (&mut self.db.local_metadata)
            .to_staged(merge_changes)
            .to_lazy()
            .promote()?;
        self.cleanup_local_metadata()?;

        Ok(())
    }

    pub(crate) fn prune_remote_orphans(
        &mut self, remote_changes: Vec<SignedFile>,
    ) -> LbResult<Vec<SignedFile>> {
        let me = Owner(self.get_public_key()?);
        let remote = self.db.base_metadata.stage(remote_changes).to_lazy();
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
        let mut local = self
            .db
            .base_metadata
            .stage(&self.db.local_metadata)
            .to_lazy();
        let base_ids = local.tree.base.owned_ids();
        let server_ids = self
            .client
            .request(self.get_account()?, GetFileIdsRequest {})?
            .ids;

        let mut prunable_ids = base_ids;
        prunable_ids.retain(|id| !server_ids.contains(id));
        for id in prunable_ids.clone() {
            prunable_ids.extend(local.descendants(&id)?.into_iter());
        }
        for id in &prunable_ids {
            if let Some(base_file) = local.tree.base.maybe_find(id) {
                self.docs.delete(id, base_file.document_hmac())?;
            }
            if let Some(local_file) = local.maybe_find(id) {
                self.docs.delete(id, local_file.document_hmac())?;
            }
        }

        let mut base_staged = (&mut self.db.base_metadata).to_lazy().stage(None);
        base_staged.tree.removed = prunable_ids.clone();
        base_staged.promote()?;

        let mut local_staged = (&mut self.db.local_metadata).to_lazy().stage(None);
        local_staged.tree.removed = prunable_ids;
        local_staged.promote()?;

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
            if !self.db.pub_key_lookup.get().contains_key(&owner) {
                let username_result = self
                    .client
                    .request(self.get_account()?, GetUsernameRequest { key: owner.0 });
                let username = match username_result {
                    Err(ApiError::Endpoint(GetUsernameError::UserNotFound)) => {
                        "<unknown>".to_string()
                    }
                    _ => username_result?.username.clone(),
                };
                self.db.pub_key_lookup.insert(owner, username.clone())?;
            }
        }

        Ok(())
    }

    // todo: check only necessary ids
    fn cleanup_local_metadata(&mut self) -> LbResult<()> {
        self.db
            .base_metadata
            .stage(&mut self.db.local_metadata)
            .prune()?;
        Ok(())
    }
}
