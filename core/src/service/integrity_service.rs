use std::path::Path;

use lockbook_shared::file_like::FileLike;
use lockbook_shared::tree_like::{Stagable, TreeLike};

use crate::model::drawing;
use crate::model::errors::{TestRepoError, Warning};
use crate::repo::document_repo;
use crate::{CoreError, OneKey, RepoSource, RequestContext};

const UTF8_SUFFIXES: [&str; 12] =
    ["md", "txt", "text", "markdown", "sh", "zsh", "bash", "html", "css", "js", "csv", "rs"];

impl RequestContext<'_, '_> {
    pub fn test_repo_integrity(&mut self) -> Result<Vec<Warning>, TestRepoError> {
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

        tree.validate()?;

        for id in tree.owned_ids() {
            let name = tree.name(&id, account)?;
            if name.is_empty() {
                return Err(TestRepoError::FileNameEmpty(id));
            }
            if name.contains('/') {
                return Err(TestRepoError::FileNameContainsSlash(id));
            }
        }

        let mut warnings = Vec::new();
        for id in tree.owned_ids() {
            let file = tree.find(&id)?;
            let doc = file.is_document();
            let cont = file.document_hmac().is_some();
            let not_deleted = !tree.calculate_deleted(&id)?;
            if not_deleted && doc && cont {
                let doc = match document_repo::maybe_get(self.config, RepoSource::Local, &id)? {
                    Some(local) => Some(local),
                    None => document_repo::maybe_get(self.config, RepoSource::Base, &id)?,
                }
                .ok_or(CoreError::FileNonexistent)?;

                let doc = tree.decrypt_document(&id, &doc, account)?;

                if doc.len() as u64 == 0 {
                    warnings.push(Warning::EmptyFile(id));
                    continue;
                }

                let file_path = tree.id_to_path(&id, account)?;
                let extension = Path::new(&file_path)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("");

                if UTF8_SUFFIXES.contains(&extension) && String::from_utf8(doc.clone()).is_err() {
                    warnings.push(Warning::InvalidUTF8(id));
                    continue;
                }

                if extension == "draw" && drawing::parse_drawing(&doc).is_err() {
                    warnings.push(Warning::UnreadableDrawing(id));
                }
            }
        }

        Ok(warnings)
    }
}
