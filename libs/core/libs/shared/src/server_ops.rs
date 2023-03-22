use crate::clock::get_time;
use crate::file_like::FileLike;
use crate::file_metadata::FileDiff;
use crate::lazy::{LazyStaged1, LazyTree};
use crate::server_file::{IntoServerFile, ServerFile};

use crate::server_tree::ServerTree;
use crate::signed_file::SignedFile;
use crate::tree_like::TreeLike;
use crate::{SharedErrorKind, SharedResult};

type LazyServerStaged1<'a> = LazyStaged1<ServerTree<'a>, Vec<ServerFile>>;

impl<'a> LazyTree<ServerTree<'a>> {
    /// Validates a diff prior to staging it. Performs individual validations, then validations that
    /// require a tree
    pub fn stage_diff(
        mut self, changes: Vec<FileDiff<SignedFile>>,
    ) -> SharedResult<LazyServerStaged1<'a>> {
        self.tree.ids.extend(changes.iter().map(|diff| *diff.id()));

        // Check new.id == old.id
        for change in &changes {
            if let Some(old) = &change.old {
                if old.id() != change.new.id() {
                    return Err(SharedErrorKind::DiffMalformed.into());
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
                        return Err(SharedErrorKind::HmacModificationInvalid.into());
                    }
                }
                None => {
                    if change.new.timestamped_value.value.document_hmac.is_some() {
                        return Err(SharedErrorKind::HmacModificationInvalid.into());
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
                        .ok_or(SharedErrorKind::OldFileNotFound)?
                        .file;
                    if current != old {
                        return Err(SharedErrorKind::OldVersionIncorrect.into());
                    }
                }
                None => {
                    if self.maybe_find(change.new.id()).is_some() {
                        return Err(SharedErrorKind::OldVersionRequired.into());
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
