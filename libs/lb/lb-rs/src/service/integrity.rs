use std::num::NonZeroUsize;
use std::thread;

use futures::{StreamExt, stream};

use crate::model::file_metadata::Owner;
use crate::model::filename::DocumentType;
use crate::model::tree_like::TreeLike;

use crate::Lb;
use crate::model::errors::{LbErrKind, LbResult, Warning};

impl Lb {
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn test_repo_integrity(&self, check_docs: bool) -> LbResult<Vec<Warning>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        if db.last_synced.get().unwrap_or(&0) != &0 && db.root.get().is_none() {
            return Err(LbErrKind::RootNonexistent)?;
        }

        tree.validate(Owner(self.keychain.get_pk()?))?;

        for id in tree.ids() {
            let name = tree.name(&id, &self.keychain)?;
            if name.is_empty() {
                return Err(LbErrKind::FileNameEmpty)?; // todo: context candidate
            }
            if name.contains('/') {
                return Err(LbErrKind::FileNameContainsSlash)?; // todo: context candidate
            }
        }

        drop(tx);

        if !check_docs {
            return Ok(vec![]);
        }

        let mut warnings = Vec::new();
        let mut tasks = vec![];
        for file in self.list_metadatas().await? {
            if file.is_document() {
                let is_text =
                    DocumentType::from_file_name_using_extension(&file.name) == DocumentType::Text;

                if is_text {
                    tasks.push(async move { (file.id, self.read_document(file.id, false).await) });
                }
            }
        }

        let mut results = stream::iter(tasks).buffer_unordered(
            thread::available_parallelism()
                .unwrap_or(NonZeroUsize::new(4).unwrap())
                .into(),
        );

        while let Some((id, res)) = results.next().await {
            let doc = res?;
            if doc.is_empty() {
                warnings.push(Warning::EmptyFile(id));
                continue;
            }

            if String::from_utf8(doc).is_err() {
                warnings.push(Warning::InvalidUTF8(id));
                continue;
            }
        }

        Ok(warnings)
    }
}
