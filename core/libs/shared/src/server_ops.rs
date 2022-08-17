use crate::clock::get_time;
use crate::file_like::FileLike;
use crate::file_metadata::{FileDiff, Owner};
use crate::lazy::{LazyStaged1, LazyTree};
use crate::server_file::{IntoServerFile, ServerFile};
use std::collections::HashSet;

use crate::access_info::UserAccessMode;
use crate::tree_like::{Stagable, TreeLike};
use crate::{SharedError, SharedResult};

impl<T> LazyTree<T>
where
    T: Stagable<F = ServerFile>,
{
    /// Validates a diff prior to staging it. Performs individual validations, then validations that
    /// require a tree
    pub fn stage_diff(
        mut self, owner: &Owner, changes: Vec<FileDiff>,
    ) -> SharedResult<LazyStaged1<T, Vec<ServerFile>>> {
        // Check new.id == old.id
        for change in &changes {
            if let Some(old) = &change.old {
                if old.id() != change.new.id() {
                    return Err(SharedError::DiffMalformed);
                }
            }
        }

        // Check for updates to root
        for change in &changes {
            if let Some(file) = self.maybe_find(change.new.id()) {
                if file.is_root() {
                    return Err(SharedError::RootModificationInvalid);
                }
            }
        }

        // Check for root creations
        for change in &changes {
            if change.new.is_root() {
                return Err(SharedError::DiffMalformed);
            }
        }

        // Check for changes to digest
        for change in &changes {
            match &change.old {
                Some(old) => {
                    if old.timestamped_value.value.document_hmac
                        != change.new.timestamped_value.value.document_hmac
                    {
                        return Err(SharedError::HmacModificationInvalid);
                    }
                }
                None => {
                    if change.new.timestamped_value.value.document_hmac.is_some() {
                        return Err(SharedError::HmacModificationInvalid);
                    }
                }
            }
        }

        // Check for race conditions
        for change in &changes {
            match &change.old {
                Some(old) => {
                    let current = &self
                        .maybe_find(old.id())
                        .ok_or(SharedError::OldFileNotFound)?
                        .file;
                    if current != old {
                        return Err(SharedError::OldVersionIncorrect);
                    }
                }
                None => {
                    if self.maybe_find(change.new.id()).is_some() {
                        return Err(SharedError::OldVersionRequired);
                    }
                }
            }
        }

        // Check ownership

        // Files that exist already must have access mode > write to edit them
        // New files are filtered so that the parent in a series of created folders is evaluated for access

        let mut files_with_old = changes.clone();
        files_with_old.retain(|change| change.old.is_some());

        let mut files_without_old = changes.clone();
        files_without_old.retain(|change| change.old.is_none());
        let mut redundant_new_files = HashSet::new();
        for file in &files_without_old {
            if files_without_old
                .iter()
                .any(|parent| file.new.parent() == parent.id())
            {
                redundant_new_files.insert(*file.id());
            }
        }
        files_without_old.retain(|f| !redundant_new_files.contains(f.new.id()));

        for change in files_with_old {
            if let Some(old) = change.old {
                if old.parent() != change.new.parent() {
                    if self.access_mode(*owner, change.new.parent())? < Some(UserAccessMode::Write)
                    {
                        return Err(SharedError::NotPermissioned);
                    }

                    if self.access_mode(*owner, old.parent())? < Some(UserAccessMode::Write) {
                        return Err(SharedError::NotPermissioned);
                    }
                }

                if self.access_mode(*owner, change.new.id())? < Some(UserAccessMode::Write) {
                    return Err(SharedError::NotPermissioned);
                }
            }
        }

        for change in files_without_old {
            if self.access_mode(*owner, change.new.parent())? < Some(UserAccessMode::Write) {
                return Err(SharedError::NotPermissioned);
            }
        }

        // Check for updates to deleted files
        for change in &changes {
            if self.maybe_find(change.new.id()).is_some()
                && self.calculate_deleted(change.new.id())?
            {
                return Err(SharedError::DeletedFileUpdated);
            }
        }

        let now = get_time().0 as u64;
        let changes = changes
            .into_iter()
            .map(|change| change.new.add_time(now))
            .collect();

        Ok(self.stage(changes))
    }
}
