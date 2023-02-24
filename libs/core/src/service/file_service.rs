use crate::{CoreState, LbErrorKind, LbResult, Requester};
use lockbook_shared::access_info::UserAccessMode;
use lockbook_shared::file::File;
use lockbook_shared::file_metadata::{FileType, Owner};
use lockbook_shared::symkey;
use lockbook_shared::tree_like::TreeLike;
use std::iter;
use uuid::Uuid;

impl<Client: Requester> CoreState<Client> {
    pub(crate) fn create_file(
        &mut self, name: &str, parent: &Uuid, file_type: FileType,
    ) -> LbResult<File> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&mut self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(LbErrorKind::AccountNonexistent)?;

        let id =
            tree.create(Uuid::new_v4(), symkey::generate_key(), parent, name, file_type, account)?;

        let ui_file = tree.finalize(&id, account, &mut self.db.pub_key_lookup)?;

        info!("created {:?} with id {id}", file_type);

        Ok(ui_file)
    }

    pub(crate) fn rename_file(&mut self, id: &Uuid, new_name: &str) -> LbResult<()> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&mut self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(LbErrorKind::AccountNonexistent)?;

        tree.rename(id, new_name, account)?;

        Ok(())
    }

    pub(crate) fn move_file(&mut self, id: &Uuid, new_parent: &Uuid) -> LbResult<()> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&mut self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(LbErrorKind::AccountNonexistent)?;

        tree.move_file(id, new_parent, account)?;
        Ok(())
    }

    pub(crate) fn delete(&mut self, id: &Uuid) -> LbResult<()> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&mut self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(LbErrorKind::AccountNonexistent)?;

        tree.delete(id, account)?;

        Ok(())
    }

    pub(crate) fn root(&mut self) -> LbResult<File> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(LbErrorKind::AccountNonexistent)?;

        let root_id = self.db.root.data().ok_or(LbErrorKind::RootNonexistent)?;

        let root = tree.finalize(root_id, account, &mut self.db.pub_key_lookup)?;

        Ok(root)
    }

    pub(crate) fn list_metadatas(&mut self) -> LbResult<Vec<File>> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(LbErrorKind::AccountNonexistent)?;

        let ids = tree.owned_ids().into_iter();

        tree.resolve_and_finalize(account, ids, &mut self.db.pub_key_lookup)
    }

    pub(crate) fn get_children(&mut self, id: &Uuid) -> LbResult<Vec<File>> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(LbErrorKind::AccountNonexistent)?;

        let ids = tree.children_using_links(id)?.into_iter();
        tree.resolve_and_finalize(account, ids, &mut self.db.pub_key_lookup)
    }

    pub(crate) fn get_and_get_children_recursively(&mut self, id: &Uuid) -> LbResult<Vec<File>> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(LbErrorKind::AccountNonexistent)?;

        let descendants = tree.descendants_using_links(id)?;
        tree.resolve_and_finalize(
            account,
            descendants.into_iter().chain(iter::once(*id)),
            &mut self.db.pub_key_lookup,
        )
    }

    pub(crate) fn get_file_by_id(&mut self, id: &Uuid) -> LbResult<File> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();

        let account = self
            .db
            .account
            .data()
            .ok_or(LbErrorKind::AccountNonexistent)?;

        if tree.calculate_deleted(id)? {
            return Err(LbErrorKind::FileNonexistent.into());
        }
        if tree.access_mode(Owner(account.public_key()), id)? < Some(UserAccessMode::Read) {
            return Err(LbErrorKind::FileNonexistent.into());
        }

        tree.finalize(id, account, &mut self.db.pub_key_lookup)
    }
}
