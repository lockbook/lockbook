use crate::model::ValidationFailure;
use crate::model::access_info::UserAccessMode;
use crate::model::file_like::FileLike;
use crate::model::file_metadata::{Diff, FileDiff, FileType, Owner};
use crate::model::filename::MAX_ENCRYPTED_FILENAME_LENGTH;
use crate::model::lazy::LazyTree;
use crate::model::staged::StagedTreeLike;
use crate::model::tree_like::TreeLike;
use std::collections::{HashMap, HashSet};

use super::errors::{LbErrKind, LbResult};

pub fn file_name(name: &str) -> LbResult<()> {
    if name.is_empty() {
        Err(LbErrKind::FileNameEmpty)?;
    }
    if name.contains('/') {
        Err(LbErrKind::FileNameContainsSlash)?;
    }
    Ok(())
}

pub fn not_root<F: FileLike>(file: &F) -> LbResult<()> {
    if file.is_root() { Err(LbErrKind::RootModificationInvalid)? } else { Ok(()) }
}

pub fn is_folder<F: FileLike>(file: &F) -> LbResult<()> {
    if file.is_folder() {
        Ok(())
    } else {
        Err(LbErrKind::Validation(ValidationFailure::NonFolderWithChildren(*file.id())))?
    }
}

pub fn is_document<F: FileLike>(file: &F) -> LbResult<()> {
    if file.is_document() { Ok(()) } else { Err(LbErrKind::FileNotDocument)? }
}

pub fn path(path: &str) -> LbResult<()> {
    if path.contains("//") || path.is_empty() {
        Err(LbErrKind::PathContainsEmptyFileName)?;
    }

    Ok(())
}

impl<T, Base, Local> LazyTree<T>
where
    T: StagedTreeLike<Base = Base, Staged = Local>,
    Base: TreeLike<F = T::F>,
    Local: TreeLike<F = T::F>,
{
    pub fn validate(&mut self, owner: Owner) -> LbResult<()> {
        // point checks
        self.assert_no_root_changes()?;
        self.assert_no_changes_to_deleted_files()?;
        self.assert_all_filenames_size_limit()?;
        self.assert_all_files_decryptable(owner)?;
        self.assert_only_folders_have_children()?;
        self.assert_all_files_same_owner_as_parent()?;

        // structure checks
        self.assert_no_cycles()?;
        self.assert_no_path_conflicts()?;
        self.assert_no_shared_links()?;
        self.assert_no_duplicate_links()?;
        self.assert_no_broken_links()?;
        self.assert_no_owned_links()?;
        self.assert_valid_attachments()?;

        // authorization check
        self.assert_changes_authorized(owner)?;

        Ok(())
    }

    // note: deleted access keys permissible
    // a deleted file whose parent left the tree has nothing left to decrypt
    pub fn assert_all_files_decryptable(&mut self, owner: Owner) -> LbResult<()> {
        for file in self.ids().into_iter().filter_map(|id| self.maybe_find(&id)) {
            if self.maybe_find_parent(file).is_none()
                && self.maybe_find_attached(file).is_none()
                && !file.explicitly_deleted()
                && !file
                    .user_access_keys()
                    .iter()
                    .any(|k| k.encrypted_for == owner.0)
            {
                Err(LbErrKind::Validation(ValidationFailure::Orphan(*file.id())))?;
            }
        }
        Ok(())
    }

    pub fn assert_all_filenames_size_limit(&self) -> LbResult<()> {
        for file in self.all_files()? {
            if file.secret_name().encrypted_value.value.len() > MAX_ENCRYPTED_FILENAME_LENGTH {
                Err(LbErrKind::Validation(ValidationFailure::FileNameTooLong(*file.id())))?;
            }
        }
        Ok(())
    }

    pub fn assert_only_folders_have_children(&self) -> LbResult<()> {
        for file in self.all_files()? {
            if let Some(parent) = self.maybe_find(file.parent()) {
                if !parent.is_folder() {
                    Err(LbErrKind::Validation(ValidationFailure::NonFolderWithChildren(
                        *parent.id(),
                    )))?;
                }
            }
        }
        Ok(())
    }

    // note: deleted files exempt because otherwise moving a folder with a deleted file in it
    // to/from a folder with a different owner would require updating a deleted file
    pub fn assert_all_files_same_owner_as_parent(&mut self) -> LbResult<()> {
        for id in self.ids() {
            if self.calculate_deleted(&id)? {
                continue;
            }
            let file = self.find(&id)?;
            if let Some(parent) = self.maybe_find(file.parent()) {
                if parent.owner() != file.owner() {
                    Err(LbErrKind::Validation(ValidationFailure::FileWithDifferentOwnerParent(
                        *file.id(),
                    )))?;
                }
            }
        }
        Ok(())
    }

    pub fn assert_valid_attachments(&mut self) -> LbResult<()> {
        let mut documents = HashSet::new();
        for id in self.ids() {
            let Some(document) = self.find(&id)?.attachments_of().map(|of| of.id) else {
                continue;
            };
            if self.find(&id)?.explicitly_deleted() || self.calculate_deleted(&id)? {
                continue;
            }
            let folder = self.find(&id)?;
            let failure = match self.maybe_find(&document) {
                _ if !folder.is_folder() => Some(ValidationFailure::InvalidAttachmentsFolder(id)),
                Some(document) if !document.is_document() => {
                    Some(ValidationFailure::InvalidAttachmentsFolder(id))
                }
                _ if !documents.insert(document) => {
                    Some(ValidationFailure::DuplicateAttachments { document })
                }
                _ => None,
            };
            if let Some(failure) = failure {
                Err(LbErrKind::Validation(failure))?;
            }
        }
        Ok(())
    }

    // assumption: no orphans
    pub fn assert_no_cycles(&mut self) -> LbResult<()> {
        let mut owners_with_found_roots = HashSet::new();
        let mut no_cycles_in_ancestors = HashSet::new();
        for id in self.ids() {
            let mut ancestors = HashSet::new();
            let mut current_file = self.find(&id)?;
            loop {
                if no_cycles_in_ancestors.contains(current_file.id()) {
                    break;
                } else if current_file.is_root() {
                    if owners_with_found_roots.insert(current_file.owner()) {
                        ancestors.insert(*current_file.id());
                        break;
                    } else {
                        Err(LbErrKind::Validation(ValidationFailure::Cycle(HashSet::from([id]))))?;
                    }
                } else if ancestors.contains(current_file.parent()) {
                    Err(LbErrKind::Validation(ValidationFailure::Cycle(
                        self.ancestors(current_file.id())?,
                    )))?;
                }
                ancestors.insert(*current_file.id());
                current_file = match self.maybe_find_parent(current_file) {
                    Some(file) => file,
                    None => {
                        if !current_file.user_access_keys().is_empty()
                            || current_file.explicitly_deleted()
                            || self.maybe_find_attached(current_file).is_some()
                        {
                            break;
                        } else {
                            return Err(LbErrKind::FileParentNonexistent)?;
                        }
                    }
                }
            }
            no_cycles_in_ancestors.extend(ancestors);
        }
        Ok(())
    }

    pub fn assert_no_path_conflicts(&mut self) -> LbResult<()> {
        let mut id_by_name = HashMap::new();
        for id in self.ids() {
            if !self.calculate_deleted(&id)? {
                let file = self.find(&id)?;
                if file.is_root() || self.maybe_find(file.parent()).is_none() {
                    continue;
                }
                if let Some(conflicting) = id_by_name.remove(file.secret_name()) {
                    Err(LbErrKind::Validation(ValidationFailure::PathConflict(HashSet::from([
                        conflicting,
                        *file.id(),
                    ]))))?;
                }
                id_by_name.insert(file.secret_name().clone(), *file.id());
            }
        }
        Ok(())
    }

    pub fn assert_no_shared_links(&self) -> LbResult<()> {
        for link in self.ids() {
            let meta = self.find(&link)?;
            if let FileType::Link { target: _ } = meta.file_type() {
                if meta.is_shared() {
                    Err(LbErrKind::Validation(ValidationFailure::SharedLink {
                        link,
                        shared_ancestor: link,
                    }))?;
                }
                for ancestor in self.ancestors(&link)? {
                    if self.find(&ancestor)?.is_shared() {
                        Err(LbErrKind::Validation(ValidationFailure::SharedLink {
                            link,
                            shared_ancestor: ancestor,
                        }))?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn assert_no_duplicate_links(&mut self) -> LbResult<()> {
        let mut linked_targets = HashSet::new();
        for link in self.ids() {
            if self.calculate_deleted(&link)? {
                continue;
            }
            if let FileType::Link { target } = self.find(&link)?.file_type() {
                if !linked_targets.insert(target) {
                    Err(LbErrKind::Validation(ValidationFailure::DuplicateLink { target }))?;
                }
            }
        }
        Ok(())
    }

    // note: a link to a deleted file is not considered broken, because then you would not be able
    // to delete a file linked to by another user.
    // note: a deleted link to a nonexistent file is not considered broken, because targets of
    // deleted links may have their shares deleted, would not appear in the server tree for a user,
    // and would be pruned from client trees
    pub fn assert_no_broken_links(&mut self) -> LbResult<()> {
        for link in self.ids() {
            if let FileType::Link { target } = self.find(&link)?.file_type() {
                if !self.calculate_deleted(&link)? && self.maybe_find(&target).is_none() {
                    Err(LbErrKind::Validation(ValidationFailure::BrokenLink(link)))?;
                }
            }
        }
        Ok(())
    }

    pub fn assert_no_owned_links(&self) -> LbResult<()> {
        for link in self.ids() {
            if let FileType::Link { target } = self.find(&link)?.file_type() {
                if let Some(target_owner) = self.maybe_find(&target).map(|f| f.owner()) {
                    if self.find(&link)?.owner() == target_owner {
                        Err(LbErrKind::Validation(ValidationFailure::OwnedLink(link)))?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn assert_no_root_changes(&mut self) -> LbResult<()> {
        for id in self.tree.staged().ids() {
            // already root
            if let Some(base) = self.tree.base().maybe_find(&id) {
                if base.is_root() {
                    Err(LbErrKind::RootModificationInvalid)?;
                }
            }
            // newly root
            if self.find(&id)?.is_root() {
                Err(LbErrKind::Validation(ValidationFailure::Cycle(
                    vec![id].into_iter().collect(),
                )))?;
            }
        }
        Ok(())
    }

    pub fn assert_no_changes_to_deleted_files(&mut self) -> LbResult<()> {
        for id in self.tree.staged().ids() {
            // already deleted files cannot have updates
            let mut base = self.tree.base().to_lazy();
            if base.maybe_find(&id).is_some() && base.calculate_deleted(&id)? {
                Err(LbErrKind::Validation(ValidationFailure::DeletedFileUpdated(id)))?;
            }
            // newly deleted files cannot have non-deletion updates
            if self.calculate_deleted(&id)? {
                if let Some(base) = self.tree.base().maybe_find(&id) {
                    if FileDiff::edit(base.clone(), self.find(&id)?.clone())
                        .diff()
                        .iter()
                        .any(|d| d != &Diff::Deleted)
                    {
                        Err(LbErrKind::Validation(ValidationFailure::DeletedFileUpdated(id)))?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn assert_changes_authorized(&mut self, owner: Owner) -> LbResult<()> {
        // Design rationale:
        // * No combination of individually valid changes should compose into an invalid change.
        //   * Owner and write access must be indistinguishable, otherwise you could e.g. move a
        //     file from write shared folder into your own, modify it in a way only owners can, then
        //     move it back. Accommodating this situation may be possible but we're not interested.
        // * Which tree - base or staged - should we check access to for an operation?
        //   * The only staged operations which cause permissions to be different in base and staged
        //     are moves and share changes. Otherwise, it doesn't matter which tree is used.
        //   * Changes by a user cannot increase the level of access of access for that user, but
        //     they can decrease it. Therefore the maximum level of access a user may have over a
        //     sequence of operations is represented in the base tree. We cannot use the staged
        //     tree in case a user removes the access they required to perform a prior operation.
        // * How do we check access for new files in new folders (which don't exist in base)?
        //   * A user will have the same access to any created folder as they do to its parent; if a
        //     user has access to create a folder, then they will have access to create its
        //     descendants and to move files such that they are descendants.
        //   * Any access checks on files with new parent folders can be skipped because the access
        //     check on the first ancestor with an existing parent folder is sufficient.
        let new_files = {
            let mut new_files = HashSet::new();
            for id in self.tree.staged().ids() {
                if self.tree.base().maybe_find(&id).is_none() {
                    new_files.insert(id);
                }
            }
            new_files
        };

        let base = self.tree.base().to_lazy();
        for file_diff in self.diffs()? {
            for field_diff in file_diff.diff() {
                match field_diff {
                    Diff::New | Diff::Name | Diff::Deleted => {
                        // use oldest version for most permissive access (see rationale)
                        let file =
                            if let Some(ref old) = file_diff.old { old } else { &file_diff.new };
                        // parent folder new -> rely on parent folder check
                        if !new_files.contains(file.parent()) {
                            // must have write access to the parent, or to the document an
                            // attachments folder belongs to
                            let through_parent = match self.maybe_find(file.parent()) {
                                Some(parent) => base.access_mode(owner, parent.id())?,
                                None => None, // this file is shared and its parent is not
                            };
                            let through_document = match base.maybe_find_attached(file) {
                                Some(document) if field_diff != Diff::New => {
                                    base.access_mode(owner, document.id())?
                                }
                                _ => None,
                            };
                            if through_parent < Some(UserAccessMode::Write)
                                && through_document < Some(UserAccessMode::Write)
                            {
                                Err(LbErrKind::InsufficientPermission)?;
                            }
                        }
                    }
                    Diff::Parent | Diff::Owner => {
                        // check access for base parent
                        {
                            let parent = if let Some(ref old) = file_diff.old {
                                old.parent()
                            } else {
                                return Err(LbErrKind::Unexpected(
                                    "Non-New FileDiff with no old".to_string(),
                                )
                                .into());
                            };

                            // must have parent and have write access to parent
                            if let Some(parent) = self.maybe_find(parent) {
                                if base.access_mode(owner, parent.id())?
                                    < Some(UserAccessMode::Write)
                                {
                                    // parent is shared with access < write
                                    Err(LbErrKind::InsufficientPermission)?;
                                }
                            } else {
                                // this file is shared and its parent is not
                                Err(LbErrKind::InsufficientPermission)?;
                            }
                        }
                        // check access for staged parent
                        {
                            let parent = file_diff.new.parent();

                            // parent folder new -> rely on parent folder check
                            if !new_files.contains(parent) {
                                // must have parent and have write access to parent
                                if let Some(parent) = self.maybe_find(parent) {
                                    if base.access_mode(owner, parent.id())?
                                        < Some(UserAccessMode::Write)
                                    {
                                        // parent is shared with access < write
                                        Err(LbErrKind::InsufficientPermission)?;
                                    }
                                } else {
                                    // this file is shared and its parent is not
                                    Err(LbErrKind::InsufficientPermission)?;
                                }
                            }
                        }
                    }
                    Diff::Hmac => {
                        // check self access
                        if base.access_mode(owner, file_diff.id())? < Some(UserAccessMode::Write) {
                            Err(LbErrKind::InsufficientPermission)?;
                        }
                    }
                    Diff::UserKeys => {
                        // any change to a file's keys takes write access, except that anyone
                        // may delete their own key
                        let base_keys = {
                            if let Some(ref old) = file_diff.old {
                                let mut base_keys = HashMap::new();
                                for key in old.user_access_keys() {
                                    base_keys.insert(
                                        (Owner(key.encrypted_by), Owner(key.encrypted_for)),
                                        (key.mode, key.deleted),
                                    );
                                }
                                base_keys
                            } else {
                                return Err(LbErrKind::Unexpected(
                                    "Non-New FileDiff with no old".to_string(),
                                ))?;
                            }
                        };
                        let has_write =
                            base.access_mode(owner, file_diff.id())? >= Some(UserAccessMode::Write);
                        let mut staged_shares = HashSet::new();
                        for key in file_diff.new.user_access_keys() {
                            let (by, for_) = (Owner(key.encrypted_by), Owner(key.encrypted_for));
                            staged_shares.insert((by, for_));
                            let base_key = base_keys.get(&(by, for_));
                            let unchanged = base_key == Some(&(key.mode, key.deleted));
                            if unchanged {
                                continue;
                            }
                            let deletes_own_key = for_ == owner
                                && key.deleted
                                && base_key
                                    .is_some_and(|&(mode, deleted)| mode == key.mode && !deleted);
                            if !has_write && !deletes_own_key {
                                Err(LbErrKind::InsufficientPermission)?;
                            }
                        }
                        let removes_key =
                            base_keys.keys().any(|share| !staged_shares.contains(share));
                        if removes_key && !has_write {
                            Err(LbErrKind::InsufficientPermission)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn diffs(&self) -> LbResult<Vec<FileDiff<Base::F>>> {
        let mut result = Vec::new();
        for id in self.tree.staged().ids() {
            let staged = self.tree.staged().find(&id)?;
            if let Some(base) = self.tree.base().maybe_find(&id) {
                result.push(FileDiff::edit(base.clone(), staged.clone()));
            } else {
                result.push(FileDiff::new(staged.clone()));
            }
        }
        Ok(result)
    }
}
