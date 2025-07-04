use super::errors::{DiffError, LbErrKind, LbResult};
use super::server_meta::{IntoServerMeta, ServerMeta};
use super::signed_meta::SignedMeta;
use crate::model::clock::get_time;
use crate::model::file_like::FileLike;
use crate::model::file_metadata::FileDiff;
use crate::model::lazy::{LazyStaged1, LazyTree};
use crate::model::server_file::{IntoServerFile, ServerFile};
use crate::model::server_tree::ServerTree;
use crate::model::signed_file::SignedFile;
use crate::model::tree_like::TreeLike;

type LazyServerStaged1<'a> = LazyStaged1<ServerTree<'a>, Vec<ServerMeta>>;

impl<'a> LazyTree<ServerTree<'a>> {
    /// Validates a diff prior to staging it. Performs individual validations, then validations that
    /// require a tree
    pub fn stage_diff(self, changes: Vec<FileDiff<SignedMeta>>) -> LbResult<LazyServerStaged1<'a>> {
        // Check new.id == old.id
        for change in &changes {
            if let Some(old) = &change.old {
                if old.id() != change.new.id() {
                    return Err(LbErrKind::Diff(DiffError::DiffMalformed))?;
                }
            }
        }

        // Check for changes to digest
        for change in &changes {
            match &change.old {
                Some(old) => {
                    if old.timestamped_value.value.document_hmac()
                        != change.new.timestamped_value.value.document_hmac()
                    {
                        return Err(LbErrKind::Diff(DiffError::HmacModificationInvalid))?;
                    }
                }
                None => {
                    if change.new.timestamped_value.value.document_hmac().is_some() {
                        return Err(LbErrKind::Diff(DiffError::HmacModificationInvalid))?;
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
                        .ok_or(LbErrKind::Diff(DiffError::OldFileNotFound))?
                        .file;
                    if current != old {
                        return Err(LbErrKind::Diff(DiffError::OldVersionIncorrect))?;
                    }
                }
                None => {
                    // if you're claiming this file is new, it must be globally unique
                    if self.tree.files.maybe_find(change.new.id()).is_some() {
                        return Err(LbErrKind::Diff(DiffError::OldVersionRequired))?;
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
