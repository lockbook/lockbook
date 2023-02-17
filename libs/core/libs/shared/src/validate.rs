use crate::access_info::UserAccessMode;
use crate::file_like::FileLike;
use crate::file_metadata::{Diff, FileDiff, FileType, Owner};
use crate::lazy::LazyTree;
use crate::staged::StagedTreeLike;
use crate::tree_like::TreeLike;
use crate::{SharedError, SharedResult, ValidationFailure};
use std::collections::{HashMap, HashSet};

pub fn file_name(name: &str) -> SharedResult<()> {
    if name.is_empty() {
        return Err(SharedError::FileNameEmpty);
    }
    if name.contains('/') {
        return Err(SharedError::FileNameContainsSlash);
    }
    Ok(())
}

pub fn not_root<F: FileLike>(file: &F) -> SharedResult<()> {
    if file.is_root() {
        Err(SharedError::RootModificationInvalid)
    } else {
        Ok(())
    }
}

pub fn is_folder<F: FileLike>(file: &F) -> SharedResult<()> {
    if file.is_folder() {
        Ok(())
    } else {
        Err(SharedError::FileNotFolder)
    }
}

pub fn is_document<F: FileLike>(file: &F) -> SharedResult<()> {
    if file.is_document() {
        Ok(())
    } else {
        Err(SharedError::FileNotDocument)
    }
}

pub fn path(path: &str) -> SharedResult<()> {
    if path.contains("//") || path.is_empty() {
        return Err(SharedError::PathContainsEmptyFileName);
    }

    Ok(())
}

impl<T, Base, Local> LazyTree<T>
where
    T: StagedTreeLike<Base = Base, Staged = Local>,
    Base: TreeLike<F = T::F>,
    Local: TreeLike<F = T::F>,
{
    pub fn validate(&mut self, owner: Owner) -> SharedResult<()> {
        // point checks
        self.assert_no_root_changes()?;
        self.assert_no_changes_to_deleted_files()?;
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

        // authorization check
        self.assert_changes_authorized(owner)?;

        Ok(())
    }

    // note: deleted access keys permissible
    pub fn assert_all_files_decryptable(&mut self, owner: Owner) -> SharedResult<()> {
        for file in self.ids().into_iter().filter_map(|id| self.maybe_find(id)) {
            if self.maybe_find_parent(file).is_none()
                && !file
                    .user_access_keys()
                    .iter()
                    .any(|k| k.encrypted_for == owner.0)
            {
                return Err(SharedError::ValidationFailure(ValidationFailure::Orphan(*file.id())));
            }
        }
        Ok(())
    }

    pub fn assert_only_folders_have_children(&self) -> SharedResult<()> {
        for file in self.all_files()? {
            if let Some(parent) = self.maybe_find(file.parent()) {
                if !parent.is_folder() {
                    return Err(SharedError::ValidationFailure(
                        ValidationFailure::NonFolderWithChildren(*parent.id()),
                    ));
                }
            }
        }
        Ok(())
    }

    // note: deleted files exempt because otherwise moving a folder with a deleted file in it
    // to/from a folder with a different owner would require updating a deleted file
    pub fn assert_all_files_same_owner_as_parent(&mut self) -> SharedResult<()> {
        for id in self.owned_ids() {
            if self.calculate_deleted(&id)? {
                continue;
            }
            let file = self.find(&id)?;
            if let Some(parent) = self.maybe_find(file.parent()) {
                if parent.owner() != file.owner() {
                    return Err(SharedError::ValidationFailure(
                        ValidationFailure::FileWithDifferentOwnerParent(*file.id()),
                    ));
                }
            }
        }
        Ok(())
    }

    // assumption: no orphans
    pub fn assert_no_cycles(&mut self) -> SharedResult<()> {
        let mut owners_with_found_roots = HashSet::new();
        let mut no_cycles_in_ancestors = HashSet::new();
        for id in self.owned_ids() {
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
                        return Err(SharedError::ValidationFailure(ValidationFailure::Cycle(
                            HashSet::from([id]),
                        )));
                    }
                } else if ancestors.contains(current_file.parent()) {
                    return Err(SharedError::ValidationFailure(ValidationFailure::Cycle(
                        self.ancestors(current_file.id())?,
                    )));
                }
                ancestors.insert(*current_file.id());
                current_file = match self.maybe_find_parent(current_file) {
                    Some(file) => file,
                    None => {
                        if !current_file.user_access_keys().is_empty() {
                            break;
                        } else {
                            return Err(SharedError::FileParentNonexistent);
                        }
                    }
                }
            }
            no_cycles_in_ancestors.extend(ancestors);
        }
        Ok(())
    }

    pub fn assert_no_path_conflicts(&mut self) -> SharedResult<()> {
        let mut id_by_name = HashMap::new();
        for id in self.owned_ids() {
            if !self.calculate_deleted(&id)? {
                let file = self.find(&id)?;
                if file.is_root() || self.maybe_find(file.parent()).is_none() {
                    continue;
                }
                if let Some(conflicting) = id_by_name.remove(file.secret_name()) {
                    return Err(SharedError::ValidationFailure(ValidationFailure::PathConflict(
                        HashSet::from([conflicting, *file.id()]),
                    )));
                }
                id_by_name.insert(file.secret_name().clone(), *file.id());
            }
        }
        Ok(())
    }

    pub fn assert_no_shared_links(&self) -> SharedResult<()> {
        for link in self.owned_ids() {
            let meta = self.find(&link)?;
            if let FileType::Link { target: _ } = meta.file_type() {
                if meta.is_shared() {
                    return Err(SharedError::ValidationFailure(ValidationFailure::SharedLink {
                        link,
                        shared_ancestor: link,
                    }));
                }
                for ancestor in self.ancestors(&link)? {
                    if self.find(&ancestor)?.is_shared() {
                        return Err(SharedError::ValidationFailure(
                            ValidationFailure::SharedLink { link, shared_ancestor: ancestor },
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn assert_no_duplicate_links(&mut self) -> SharedResult<()> {
        let mut linked_targets = HashSet::new();
        for link in self.owned_ids() {
            if self.calculate_deleted(&link)? {
                continue;
            }
            if let FileType::Link { target } = self.find(&link)?.file_type() {
                if !linked_targets.insert(target) {
                    return Err(SharedError::ValidationFailure(ValidationFailure::DuplicateLink {
                        target,
                    }));
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
    pub fn assert_no_broken_links(&mut self) -> SharedResult<()> {
        for link in self.owned_ids() {
            if let FileType::Link { target } = self.find(&link)?.file_type() {
                if !self.calculate_deleted(&link)? && self.maybe_find(&target).is_none() {
                    return Err(SharedError::ValidationFailure(ValidationFailure::BrokenLink(
                        link,
                    )));
                }
            }
        }
        Ok(())
    }

    pub fn assert_no_owned_links(&self) -> SharedResult<()> {
        for link in self.owned_ids() {
            if let FileType::Link { target } = self.find(&link)?.file_type() {
                if let Some(target_owner) = self.maybe_find(&target).map(|f| f.owner()) {
                    if self.find(&link)?.owner() == target_owner {
                        return Err(SharedError::ValidationFailure(ValidationFailure::OwnedLink(
                            link,
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn assert_no_root_changes(&mut self) -> SharedResult<()> {
        for id in self.tree.staged().owned_ids() {
            // already root
            if let Some(base) = self.tree.base().maybe_find(&id) {
                if base.is_root() {
                    return Err(SharedError::RootModificationInvalid);
                }
            }
            // newly root
            if self.find(&id)?.is_root() {
                return Err(SharedError::ValidationFailure(ValidationFailure::Cycle(
                    vec![id].into_iter().collect(),
                )));
            }
        }
        Ok(())
    }

    pub fn assert_no_changes_to_deleted_files(&mut self) -> SharedResult<()> {
        for id in self.tree.staged().owned_ids() {
            // already deleted files cannot have updates
            let mut base = self.tree.base().to_lazy();
            if base.maybe_find(&id).is_some() && base.calculate_deleted(&id)? {
                return Err(SharedError::DeletedFileUpdated(id));
            }
            // newly deleted files cannot have non-deletion updates
            if self.calculate_deleted(&id)? {
                if let Some(base) = self.tree.base().maybe_find(&id) {
                    if FileDiff::edit(&base, &self.find(&id)?)
                        .diff()
                        .iter()
                        .any(|d| d != &Diff::Deleted)
                    {
                        return Err(SharedError::DeletedFileUpdated(id));
                    }
                }
            }
        }
        Ok(())
    }

    pub fn assert_changes_authorized(&mut self, owner: Owner) -> SharedResult<()> {
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
            for id in self.tree.staged().owned_ids() {
                if self.tree.base().maybe_find(&id).is_none() {
                    new_files.insert(id);
                }
            }
            new_files
        };

        for file_diff in self.diffs()? {
            for field_diff in file_diff.diff() {
                match field_diff {
                    Diff::New | Diff::Name | Diff::Deleted => {
                        // use oldest version for most permissive access (see rationale)
                        let file =
                            if let Some(ref old) = file_diff.old { old } else { &file_diff.new };
                        // parent folder new -> rely on parent folder check
                        if !new_files.contains(file.parent()) {
                            // must have parent and have write access to parent
                            if let Some(parent) = self.maybe_find(file.parent()) {
                                if self.access_mode(owner, parent.id())?
                                    < Some(UserAccessMode::Write)
                                {
                                    // parent is shared with access < write
                                    return Err(SharedError::InsufficientPermission);
                                }
                            } else {
                                // this file is shared and its parent is not
                                return Err(SharedError::InsufficientPermission);
                            }
                        }
                    }
                    Diff::Parent | Diff::Owner => {
                        // check access for base parent
                        {
                            let parent = if let Some(ref old) = file_diff.old {
                                old.parent()
                            } else {
                                return Err(SharedError::Unexpected(
                                    "Non-New FileDiff with no old",
                                ));
                            };

                            // must have parent and have write access to parent
                            if let Some(parent) = self.maybe_find(parent) {
                                if self.access_mode(owner, parent.id())?
                                    < Some(UserAccessMode::Write)
                                {
                                    // parent is shared with access < write
                                    return Err(SharedError::InsufficientPermission);
                                }
                            } else {
                                // this file is shared and its parent is not
                                return Err(SharedError::InsufficientPermission);
                            }
                        }
                        // check access for staged parent
                        {
                            let parent = file_diff.new.parent();

                            // parent folder new -> rely on parent folder check
                            if !new_files.contains(parent) {
                                // must have parent and have write access to parent
                                if let Some(parent) = self.maybe_find(parent) {
                                    if self.access_mode(owner, parent.id())?
                                        < Some(UserAccessMode::Write)
                                    {
                                        // parent is shared with access < write
                                        return Err(SharedError::InsufficientPermission);
                                    }
                                } else {
                                    // this file is shared and its parent is not
                                    return Err(SharedError::InsufficientPermission);
                                }
                            }
                        }
                    }
                    Diff::Hmac => {
                        // check self access
                        if self.access_mode(owner, file_diff.id())? < Some(UserAccessMode::Write) {
                            return Err(SharedError::InsufficientPermission);
                        }
                    }
                    Diff::UserKeys => {
                        // change access: either changing your own access, or have write access
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
                                return Err(SharedError::Unexpected(
                                    "Non-New FileDiff with no old",
                                ));
                            }
                        };
                        for key in file_diff.new.user_access_keys() {
                            if let Some((base_mode, base_deleted)) =
                                base_keys.get(&(Owner(key.encrypted_by), Owner(key.encrypted_for)))
                            {
                                // editing an existing share

                                let (staged_mode, staged_deleted) = (&key.mode, &key.deleted);
                                // cannot delete someone else's share without write access
                                if *staged_deleted
                                    && !*base_deleted
                                    && self.access_mode(owner, file_diff.id())?
                                        < Some(UserAccessMode::Write)
                                    && owner.0 != key.encrypted_for
                                {
                                    return Err(SharedError::InsufficientPermission);
                                }
                                // cannot grant yourself write access
                                if staged_mode != base_mode
                                    && self.access_mode(owner, file_diff.id())?
                                        < Some(UserAccessMode::Write)
                                {
                                    return Err(SharedError::InsufficientPermission);
                                }
                            } else {
                                // adding a new share

                                // to add a share, need equal access
                                if self.access_mode(owner, file_diff.id())? < Some(key.mode) {
                                    return Err(SharedError::InsufficientPermission);
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn diffs(&self) -> SharedResult<Vec<FileDiff<Base::F>>> {
        let mut result = Vec::new();
        for id in self.tree.staged().owned_ids() {
            let staged = self.tree.staged().find(&id)?;
            if let Some(base) = self.tree.base().maybe_find(&id) {
                result.push(FileDiff::edit(base, staged));
            } else {
                result.push(FileDiff::new(staged));
            }
        }
        Ok(result)
    }
}
