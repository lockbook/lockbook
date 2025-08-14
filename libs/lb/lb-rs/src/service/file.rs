use crate::Lb;
use crate::model::access_info::UserAccessMode;
use crate::model::errors::{LbErrKind, LbResult};
use crate::model::file::File;
use crate::model::file_metadata::{FileType, Owner};
use crate::model::filename::MAX_FILENAME_LENGTH;
use crate::model::symkey;
use crate::model::tree_like::TreeLike;
use crate::LbServer;
use std::iter;
use uuid::Uuid;

impl LbServer {
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn create_file(
        &self, name: &str, parent: &Uuid, file_type: FileType,
    ) -> LbResult<File> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        // todo this is checked later and probably can be removed
        if name.len() > MAX_FILENAME_LENGTH {
            return Err(LbErrKind::FileNameTooLong.into());
        }
        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();

        let id = tree.create(
            Uuid::new_v4(),
            symkey::generate_key(),
            parent,
            name,
            file_type,
            &self.keychain,
        )?;

        let ui_file = tree.decrypt(&self.keychain, &id, &db.pub_key_lookup)?;

        tx.end();

        self.events.meta_changed();
        Ok(ui_file)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn rename_file(&self, id: &Uuid, new_name: &str) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        if new_name.len() > MAX_FILENAME_LENGTH {
            return Err(LbErrKind::FileNameTooLong.into());
        }
        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();

        let id = &tree.linked_by(id)?.unwrap_or(*id);

        tree.rename(id, new_name, &self.keychain)?;

        tx.end();

        self.events.meta_changed();

        Ok(())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn move_file(&self, id: &Uuid, new_parent: &Uuid) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();

        let id = &tree.linked_by(id)?.unwrap_or(*id);

        tree.move_file(id, new_parent, &self.keychain)?;
        tx.end();

        self.events.meta_changed();

        Ok(())
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn delete(&self, id: &Uuid) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();

        let id = &tree.linked_by(id)?.unwrap_or(*id);

        tree.delete(id, &self.keychain)?;

        tx.end();

        self.events.meta_changed();

        Ok(())
    }

    // todo: keychain?
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn root(&self) -> LbResult<File> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let root_id = db.root.get().ok_or(LbErrKind::RootNonexistent)?;

        let root = tree.decrypt(&self.keychain, root_id, &db.pub_key_lookup)?;

        Ok(root)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn list_metadatas(&self) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let ids = tree.ids().into_iter();

        tree.decrypt_all(&self.keychain, ids, &db.pub_key_lookup, true)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn get_children(&self, id: &Uuid) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let ids = tree.children_using_links(id)?.into_iter();

        tree.decrypt_all(&self.keychain, ids, &db.pub_key_lookup, true)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn get_and_get_children_recursively(&self, id: &Uuid) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let descendants = tree.descendants_using_links(id)?;

        tree.decrypt_all(
            &self.keychain,
            descendants.into_iter().chain(iter::once(*id)),
            &db.pub_key_lookup,
            true,
        )
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn get_file_by_id(&self, id: Uuid) -> LbResult<File> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        if tree.calculate_deleted(&id)? {
            return Err(LbErrKind::FileNonexistent.into());
        }
        if tree.access_mode(Owner(self.keychain.get_pk()?), &id)? < Some(UserAccessMode::Read) {
            return Err(LbErrKind::FileNonexistent.into());
        }

        let file = tree.decrypt(&self.keychain, &id, &db.pub_key_lookup)?;

        Ok(file)
    }

    pub async fn local_changes(&self) -> Vec<Uuid> {
        let tx = self.ro_tx().await;
        let db = tx.db();
        db.local_metadata.get().keys().copied().collect()
    }
}
