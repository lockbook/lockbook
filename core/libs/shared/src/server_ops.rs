use crate::clock::get_time;
use crate::file::like::FileLike;
use crate::file::metadata::{FileDiff, Owner};
use crate::file::server::{IntoServerFile, ServerFile};
use crate::tree::lazy::LazyStaged1;
use hmdb::log::SchemaEvent;
use std::collections::HashSet;
use uuid::Uuid;

use crate::file::signed::SignedFile;
use crate::tree::like::TreeLike;
use crate::tree::server::ServerTree;
use crate::{SharedError, SharedResult};

type LazyServerStaged1<'a, 'b, 'v, OwnedFiles, SharedFiles, FileChildren, Files> = LazyStaged1<
    'v,
    ServerTree<'a, 'b, OwnedFiles, SharedFiles, FileChildren, Files>,
    Vec<ServerFile>,
>;

impl<'a, 'b, 'v, OwnedFiles, SharedFiles, FileChildren, Files>
    LazyServerStaged1<'a, 'b, 'v, OwnedFiles, SharedFiles, FileChildren, Files>
where
    OwnedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    SharedFiles: SchemaEvent<Owner, HashSet<Uuid>>,
    FileChildren: SchemaEvent<Uuid, HashSet<Uuid>>,
    Files: SchemaEvent<Uuid, ServerFile>,
{
    /// Validates a diff prior to staging it. Performs individual validations, then validations that
    /// require a tree
    pub fn stage_diff(mut self, changes: Vec<FileDiff<SignedFile>>) -> SharedResult<Self> {
        self.tree
            .base
            .ids
            .extend(changes.iter().map(|diff| *diff.id()));

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
        *self.tree.staged = changes
            .into_iter()
            .map(|change| change.new.add_time(now))
            .collect();

        Ok(self)
    }
}
