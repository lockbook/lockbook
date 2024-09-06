use crate::logic::clock::get_time;
use crate::logic::crypto::DecryptedDocument;
use crate::logic::file_like::FileLike;
use crate::logic::file_metadata::FileType;
use crate::logic::tree_like::TreeLike;
use crate::model::errors::{CoreError, LbResult};
use crate::Lb;
use uuid::Uuid;

use super::activity;

impl Lb {
    pub async fn read_document(&self, id: Uuid) -> LbResult<DecryptedDocument> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let account = self.get_account()?;

        //let doc = tree.read_document(&self.async_docs, &id, account).await?;

        //self.add_doc_event(activity_service::DocEvent::Read(id, get_time().0))?;

        //Ok(doc)
        todo!()
    }

    pub async fn write_document(&self, id: Uuid, content: &[u8]) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();
        let account = db.account.get().ok_or(CoreError::AccountNonexistent)?;

        let id = match tree.find(&id)?.file_type() {
            FileType::Document | FileType::Folder => id,
            FileType::Link { target } => target,
        };
        let encrypted_document = tree.update_document(&id, content, account)?;
        let hmac = tree.find(&id)?.document_hmac();
        self.docs.insert(&id, hmac, &encrypted_document).await?;
        tx.end();

        let bg_lb = self.clone();
        tokio::spawn(async move {
            bg_lb
                .add_doc_event(activity::DocEvent::Write(id, get_time().0))
                .await
                .unwrap();
            bg_lb.cleanup().await.unwrap();
        });

        Ok(())
    }

    pub(crate) async fn cleanup(&self) -> LbResult<()> {
        if false {
            // todo need to think out this business now
            debug!("skipping doc cleanup due to active sync");
            return Ok(());
        }

        // todo
        // self.db
        //     .base_metadata
        //     .stage(&mut self.db.local_metadata)
        //     .to_lazy()
        //     .delete_unreferenced_file_versions(&self.docs)?;
        Ok(())
    }
}
