use crate::model::drawing;
use crate::model::errors::{TestRepoError, Warning};
use crate::{Config, OneKey, RequestContext};
use lockbook_shared::tree_like::Stagable;
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::{SharedError, ValidationFailure};
use std::path::Path;

const UTF8_SUFFIXES: [&str; 12] =
    ["md", "txt", "text", "markdown", "sh", "zsh", "bash", "html", "css", "js", "csv", "rs"];

impl RequestContext<'_, '_> {
    pub fn test_repo_integrity(&mut self, config: &Config) -> Result<Vec<Warning>, TestRepoError> {
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(TestRepoError::NoAccount)?;

        let mut tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();

        if self.tx.last_synced.get(&OneKey {}).unwrap_or(&0) != &0
            && self.tx.root.get(&OneKey).is_none()
        {
            return Err(TestRepoError::NoRootFolder);
        }

        tree.validate().map_err(|err: SharedError| match err {
            SharedError::ValidationFailure(validation) => match validation {
                ValidationFailure::Orphan(id) => TestRepoError::FileOrphaned(id),
                ValidationFailure::Cycle(ids) => TestRepoError::CycleDetected(ids),
                ValidationFailure::PathConflict(ids) => TestRepoError::PathConflict(ids),
            },
            _ => TestRepoError::Shared(err),
        })?;

        for id in tree.ids() {
            // Find empty file names here
            // Find names with a slash here
        }

        let mut warnings = Vec::new();
        // for (id, file) in files.filter_not_deleted().map_err(TestRepoError::Tree)? {
        //     if file.is_document() {
        //         let file_content = file_service::get_document(config, RepoSource::Local, &file)
        //             .map_err(|err| DocumentReadError(id, err))?;
        //
        //         if file_content.len() as u64 == 0 {
        //             warnings.push(Warning::EmptyFile(id));
        //             continue;
        //         }
        //
        //         let file_path = self.get_path_by_id(id).map_err(TestRepoError::Core)?;
        //         let extension = Path::new(&file_path)
        //             .extension()
        //             .and_then(|ext| ext.to_str())
        //             .unwrap_or("");
        //
        //         if UTF8_SUFFIXES.contains(&extension) && String::from_utf8(file_content).is_err() {
        //             warnings.push(Warning::InvalidUTF8(id));
        //             continue;
        //         }
        //
        //         if extension == "draw"
        //             && drawing::parse_drawing(
        //                 &file_service::get_document(config, RepoSource::Local, &file)
        //                     .map_err(TestRepoError::Core)?,
        //             )
        //             .is_err()
        //         {
        //             warnings.push(Warning::UnreadableDrawing(id));
        //         }
        //     }
        // }

        Ok(warnings)
    }
}
