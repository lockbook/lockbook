use crate::clock::get_time;
use crate::file_like::FileLike;
use crate::file_metadata::{FileDiff, Owner};
use crate::lazy::{LazyStaged1, LazyTree};
use crate::server_file::{IntoServerFile, ServerFile};
use hmdb::log::SchemaEvent;
use std::collections::HashSet;
use uuid::Uuid;

use crate::server_tree::ServerTree;
use crate::signed_file::SignedFile;
use crate::tree_like::TreeLike;
use crate::{SharedError, SharedResult};

impl<'a, 'b, Log1, Log2> LazyTree<ServerTree<'a, 'b, Log1, Log2>>
where
    Log1: SchemaEvent<Owner, HashSet<Uuid>>,
    Log2: SchemaEvent<Uuid, ServerFile>,
{
    /// Validates a diff prior to staging it. Performs individual validations, then validations that
    /// require a tree
    pub fn stage_diff(
        mut self, changes: Vec<FileDiff<SignedFile>>,
    ) -> SharedResult<LazyStaged1<ServerTree<'a, 'b, Log1, Log2>, Vec<ServerFile>>> {
        self.tree.ids.extend(changes.iter().map(|diff| *diff.id()));

        // Check new.id == old.id
        for change in &changes {
            if let Some(old) = &change.old {
                if old.id() != change.new.id() {
                    return Err(SharedError::DiffMalformed);
                }
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

        let now = get_time().0 as u64;
        let changes = changes
            .into_iter()
            .map(|change| change.new.add_time(now))
            .collect();

        Ok(self.stage(changes))
    }
}
