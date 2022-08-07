use crate::clock::get_time;
use crate::file_like::FileLike;
use crate::file_metadata::{FileDiff, Owner};
use crate::lazy::{LazyStaged1, LazyTree};
use crate::server_file::{IntoServerFile, ServerFile};

use crate::tree_like::{Stagable, TreeLike};
use crate::{SharedError, SharedResult};

impl<T> LazyTree<T>
where
    T: Stagable<F = ServerFile>,
{
    pub fn stage_diff(
        mut self, owner: &Owner, changes: Vec<FileDiff>,
    ) -> SharedResult<LazyStaged1<T, Vec<ServerFile>>> {
        // Check ownership
        for change in &changes {
            if let Some(old) = &change.old {
                if old.public_key != owner.0 || &old.timestamped_value.value.owner != owner {
                    return Err(SharedError::NotPermissioned);
                }
            }

            if change.new.public_key != owner.0
                || &change.new.timestamped_value.value.owner != owner
            {
                return Err(SharedError::NotPermissioned);
            }
        }

        // Check new.id == old.id
        for change in &changes {
            if let Some(old) = &change.old {
                if old.id() != change.new.id() {
                    return Err(SharedError::DiffMalformed);
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

        // Check for updates to deleted files
        for change in &changes {
            if self.maybe_find(change.new.id()).is_some()
                && self.calculate_deleted(change.new.id())?
            {
                return Err(SharedError::DeletedFileUpdated);
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

        // Check for updates to root
        for change in &changes {
            if let Some(file) = self.maybe_find(change.new.id()) {
                if file.is_root() {
                    return Err(SharedError::RootModificationInvalid);
                }
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
