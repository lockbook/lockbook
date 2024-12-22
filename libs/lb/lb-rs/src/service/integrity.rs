use std::path::Path;

use crate::logic::file_like::FileLike;
use crate::logic::tree_like::TreeLike;
use crate::model::file_metadata::Owner;

use crate::model::errors::{TestRepoError, Warning};
use crate::Lb;

const UTF8_SUFFIXES: [&str; 12] =
    ["md", "txt", "text", "markdown", "sh", "zsh", "bash", "html", "css", "js", "csv", "rs"];

impl Lb {
    // todo good contender for async treatment, to speedup debug_info
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn test_repo_integrity(&self) -> Result<Vec<Warning>, TestRepoError> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        if db.last_synced.get().unwrap_or(&0) != &0 && db.root.get().is_none() {
            return Err(TestRepoError::NoRootFolder);
        }

        tree.validate(Owner(self.keychain.get_pk()?))?;

        for id in tree.ids() {
            let name = tree.name(&id, &self.keychain)?;
            if name.is_empty() {
                return Err(TestRepoError::FileNameEmpty(id));
            }
            if name.contains('/') {
                return Err(TestRepoError::FileNameContainsSlash(id));
            }
        }

        let mut warnings = Vec::new();
        for id in tree.ids() {
            let file = tree.find(&id)?;
            let id = *file.id();
            let doc = file.is_document();
            let cont = file.document_hmac().is_some();
            let not_deleted = !tree.calculate_deleted(&id)?;
            if not_deleted && doc && cont {
                let doc = self.read_document_helper(id, &mut tree).await?;

                if doc.len() as u64 == 0 {
                    warnings.push(Warning::EmptyFile(id));
                    continue;
                }

                let name = tree.name(&id, &self.keychain)?;
                let extension = Path::new(&name)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .unwrap_or("");

                if UTF8_SUFFIXES.contains(&extension) && String::from_utf8(doc.clone()).is_err() {
                    warnings.push(Warning::InvalidUTF8(id));
                    continue;
                }
            }
        }

        Ok(warnings)
    }
}
