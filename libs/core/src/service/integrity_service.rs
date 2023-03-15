use std::path::Path;

use lockbook_shared::file_like::FileLike;
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::tree_like::TreeLike;

use crate::model::drawing;
use crate::model::errors::{TestRepoError, Warning};
use crate::{CoreState, Requester};

const UTF8_SUFFIXES: [&str; 12] =
    ["md", "txt", "text", "markdown", "sh", "zsh", "bash", "html", "css", "js", "csv", "rs"];

impl<Client: Requester> CoreState<Client> {
    pub(crate) fn test_repo_integrity(&mut self) -> Result<Vec<Warning>, TestRepoError> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self.db.account.data().ok_or(TestRepoError::NoAccount)?;

        if self.db.last_synced.data().unwrap_or(&0) != &0 && self.db.root.data().is_none() {
            return Err(TestRepoError::NoRootFolder);
        }

        tree.validate(Owner(account.public_key()))?;

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
                let doc = tree.read_document(&self.config, &id, account)?;

                if doc.len() as u64 == 0 {
                    warnings.push(Warning::EmptyFile(id));
                    continue;
                }

                let name = tree.name(&id, account)?;
                let extension = Path::new(&name)
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
