use crate::{CoreError, CoreResult, CoreState, Requester};
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
    ) -> CoreResult<File> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&mut self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        let id =
            tree.create(Uuid::new_v4(), symkey::generate_key(), parent, name, file_type, account)?;

        let ui_file = tree
            .resolve_and_finalize_all(account, [id].into_iter(), &mut self.db.pub_key_lookup)?
            .get(0)
            .ok_or_else(|| CoreError::InvalidFinalization)?
            .to_owned();

        info!("created {:?} with id {id}", file_type);

        Ok(ui_file)
    }

    pub(crate) fn rename_file(&mut self, id: &Uuid, new_name: &str) -> CoreResult<()> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&mut self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        tree.rename(id, new_name, account)?;

        Ok(())
    }

    pub(crate) fn move_file(&mut self, id: &Uuid, new_parent: &Uuid) -> CoreResult<()> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&mut self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        tree.move_file(id, new_parent, account)?;
        Ok(())
    }

    pub(crate) fn delete(&mut self, id: &Uuid) -> CoreResult<()> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&mut self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        tree.delete(id, account)?;

        Ok(())
    }

    pub(crate) fn root(&mut self) -> CoreResult<File> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        let root_id = self.db.root.data().ok_or(CoreError::RootNonexistent)?;

        let root = tree
            .resolve_and_finalize(account, [*root_id].into_iter(), &mut self.db.pub_key_lookup)?
            .get(0)
            .ok_or_else(|| CoreError::InvalidFinalization)?
            .to_owned();

        Ok(root)
    }

    pub(crate) fn list_metadatas(&mut self) -> CoreResult<Vec<File>> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        let ids = tree.owned_ids().into_iter();

        Ok(tree.resolve_and_finalize(account, ids, &mut self.db.pub_key_lookup)?)
    }

    pub(crate) fn get_children(&mut self, id: &Uuid) -> CoreResult<Vec<File>> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        let ids = tree.children_using_links(id)?.into_iter();
        Ok(tree.resolve_and_finalize(account, ids, &mut self.db.pub_key_lookup)?)
    }

    pub(crate) fn get_and_get_children_recursively(&mut self, id: &Uuid) -> CoreResult<Vec<File>> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        let descendants = tree.descendants_using_links(id)?;
        Ok(tree.resolve_and_finalize(
            account,
            descendants.into_iter().chain(iter::once(*id)),
            &mut self.db.pub_key_lookup,
        )?)
    }

    pub(crate) fn get_file_by_id(&mut self, id: &Uuid) -> CoreResult<File> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();

        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        if tree.calculate_deleted(id)? {
            return Err(CoreError::FileNonexistent);
        }
        if tree.access_mode(Owner(account.public_key()), id)? < Some(UserAccessMode::Read) {
            return Err(CoreError::FileNonexistent);
        }

        let file = tree
            .resolve_and_finalize_all(account, [*id].into_iter(), &mut self.db.pub_key_lookup)?
            .get(0)
            .ok_or_else(|| CoreError::InvalidFinalization)?
            .to_owned();

        Ok(file)
    }
}
