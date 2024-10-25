use crate::logic::filename::MAX_FILENAME_LENGTH;
use crate::logic::symkey;
use crate::logic::tree_like::TreeLike;
use crate::model::access_info::UserAccessMode;
use crate::model::errors::{LbErrKind, LbResult};
use crate::model::file::File;
use crate::model::file_metadata::{FileType, Owner};
use crate::Lb;
use std::iter;
use uuid::Uuid;

impl Lb {
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

        let account = db.account.get().ok_or(LbErrKind::AccountNonexistent)?;

        let id =
            tree.create(Uuid::new_v4(), symkey::generate_key(), parent, name, file_type, account)?;

        let ui_file = tree.decrypt(account, &id, &db.pub_key_lookup)?;

        info!("created {:?} with id {id}", file_type);

        self.spawn_build_index();

        Ok(ui_file)
    }

    pub async fn rename_file(&self, id: &Uuid, new_name: &str) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        if new_name.len() > MAX_FILENAME_LENGTH {
            return Err(LbErrKind::FileNameTooLong.into());
        }
        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();
        let account = self.get_account()?;

        let id = &tree.linked_by(id)?.unwrap_or(*id);

        tree.rename(id, new_name, account)?;

        self.spawn_build_index();

        Ok(())
    }

    pub async fn move_file(&self, id: &Uuid, new_parent: &Uuid) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();
        let account = self.get_account()?;

        let id = &tree.linked_by(id)?.unwrap_or(*id);

        tree.move_file(id, new_parent, account)?;

        self.spawn_build_index();

        Ok(())
    }

    pub async fn delete(&self, id: &Uuid) -> LbResult<()> {
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();
        let account = self.get_account()?;

        let id = &tree.linked_by(id)?.unwrap_or(*id);

        tree.delete(id, account)?;

        self.spawn_build_index();

        Ok(())
    }

    // todo: keychain?
    pub async fn root(&self) -> LbResult<File> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let account = self.get_account()?;

        let root_id = db.root.get().ok_or(LbErrKind::RootNonexistent)?;

        let root = tree.decrypt(account, root_id, &db.pub_key_lookup)?;

        Ok(root)
    }

    pub async fn list_metadatas(&self) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let account = self.get_account()?;

        let ids = tree.owned_ids().into_iter();

        Ok(tree.decrypt_all(account, ids, &db.pub_key_lookup, true)?)
    }

    pub async fn get_children(&self, id: &Uuid) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let account = self.get_account()?;

        let ids = tree.children_using_links(id)?.into_iter();

        Ok(tree.decrypt_all(account, ids, &db.pub_key_lookup, true)?)
    }

    pub async fn get_and_get_children_recursively(&self, id: &Uuid) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let account = self.get_account()?;

        let descendants = tree.descendants_using_links(id)?;

        Ok(tree.decrypt_all(
            account,
            descendants.into_iter().chain(iter::once(*id)),
            &db.pub_key_lookup,
            false,
        )?)
    }

    pub async fn get_file_by_id(&self, id: Uuid) -> LbResult<File> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let account = self.get_account()?;

        if tree.calculate_deleted(&id)? {
            return Err(LbErrKind::FileNonexistent.into());
        }
        if tree.access_mode(Owner(self.get_pk()?), &id)? < Some(UserAccessMode::Read) {
            return Err(LbErrKind::FileNonexistent.into());
        }

        let file = tree.decrypt(account, &id, &db.pub_key_lookup)?;

        Ok(file)
    }
}
