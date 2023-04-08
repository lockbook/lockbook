use crate::{CoreError, CoreState, LbResult, Requester};
use lockbook_shared::access_info::UserAccessMode;
use lockbook_shared::file::File;
use lockbook_shared::file_metadata::{FileType, Owner};
use lockbook_shared::filename::MAX_FILENAME_LENGTH;
use lockbook_shared::symkey;
use lockbook_shared::tree_like::TreeLike;
use std::iter;
use uuid::Uuid;

impl<Client: Requester> CoreState<Client> {
    pub(crate) fn create_file(
        &mut self, name: &str, parent: &Uuid, file_type: FileType,
    ) -> LbResult<File> {
        if name.len() > MAX_FILENAME_LENGTH {
            return Err(CoreError::FileNameTooLong.into());
        }
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

        let mut ui_file = tree.finalize(account, id, &mut self.db.pub_key_lookup)?;
        if matches!(file_type, FileType::Link { .. }) {
            ui_file.id = id;
        }

        info!("created {:?} with id {id}", file_type);

        Ok(ui_file)
    }

    pub(crate) fn rename_file(&mut self, id: &Uuid, new_name: &str) -> LbResult<()> {
        if new_name.len() > MAX_FILENAME_LENGTH {
            return Err(CoreError::FileNameTooLong.into());
        }
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

    pub(crate) fn move_file(&mut self, id: &Uuid, new_parent: &Uuid) -> LbResult<()> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&mut self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        let id = match tree.link(id)? {
            None => *id,
            Some(target) => target,
        };
        tree.move_file(&id, new_parent, account)?;

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
            .ok_or(CoreError::AccountNonexistent)?;

        let id = match tree.link(id)? {
            None => *id,
            Some(target) => target,
        };

        tree.delete(&id, account)?;

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
            .ok_or(CoreError::AccountNonexistent)?;

        let root_id = self.db.root.data().ok_or(CoreError::RootNonexistent)?;

        let root = tree.finalize(account, *root_id, &mut self.db.pub_key_lookup)?;

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
            .ok_or(CoreError::AccountNonexistent)?;

        let ids = tree.owned_ids().into_iter();

        Ok(tree.finalize_all(account, ids, &mut self.db.pub_key_lookup, true)?)
    }

    pub(crate) fn get_children(&mut self, id: &Uuid) -> LbResult<Vec<File>> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        let ids = tree.children_using_links(id)?.into_iter();

        Ok(tree.finalize_all(account, ids, &mut self.db.pub_key_lookup, true)?)
    }

    pub(crate) fn get_and_get_children_recursively(&mut self, id: &Uuid) -> LbResult<Vec<File>> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();
        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        let descendants = tree.descendants_using_links(id)?;

        Ok(tree.finalize_all(
            account,
            descendants.into_iter().chain(iter::once(*id)),
            &mut self.db.pub_key_lookup,
            false,
        )?)
    }

    pub(crate) fn get_file_by_id(&mut self, id: &Uuid) -> LbResult<File> {
        let mut tree = (&self.db.base_metadata)
            .to_staged(&self.db.local_metadata)
            .to_lazy();

        let account = self
            .db
            .account
            .data()
            .ok_or(CoreError::AccountNonexistent)?;

        if tree.calculate_deleted(id)? {
            return Err(CoreError::FileNonexistent.into());
        }
        if tree.access_mode(Owner(account.public_key()), id)? < Some(UserAccessMode::Read) {
            return Err(CoreError::FileNonexistent.into());
        }

        let file = tree.finalize(account, *id, &mut self.db.pub_key_lookup)?;

        Ok(file)
    }
}
