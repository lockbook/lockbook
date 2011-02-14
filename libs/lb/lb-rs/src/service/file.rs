use crate::LocalLb;
use crate::model::access_info::UserAccessMode;
use crate::model::errors::{LbErrKind, LbResult};
use crate::model::file::File;
use crate::model::file_like::FileLike;
use crate::model::file_metadata::{FileType, Owner};
use crate::model::filename::{MAX_FILENAME_LENGTH, NameComponents};
use crate::model::symkey;
use crate::model::tree_like::TreeLike;
use crate::service::events::Actor;
use std::iter;
use uuid::Uuid;

impl LocalLb {
    /// Creates independent copies of files in `parent`.
    ///
    /// Folders are copied recursively and links are materialized as copies of
    /// their targets so changing a duplicate can never change the source.
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn duplicate_files(&self, ids: &[Uuid], parent: &Uuid) -> LbResult<Vec<File>> {
        let mut duplicates = Vec::with_capacity(ids.len());

        for id in ids {
            let source = self.get_file_by_id(*id).await?;
            let (duplicate, duplicate_source_id) = self.duplicate_one(source, parent).await?;
            let mut pending_folders = vec![(duplicate_source_id, duplicate.id)];

            while let Some((source_id, duplicate_parent)) = pending_folders.pop() {
                let source = self.get_file_by_id(source_id).await?;
                if !source.is_folder() {
                    continue;
                }

                for child in self.get_children(&source_id).await? {
                    let (duplicate_child, duplicate_source_id) =
                        self.duplicate_one(child.clone(), &duplicate_parent).await?;
                    if duplicate_child.is_folder() {
                        pending_folders.push((duplicate_source_id, duplicate_child.id));
                    }
                }
            }

            duplicates.push(duplicate);
        }

        Ok(duplicates)
    }

    async fn duplicate_one(&self, source: File, parent: &Uuid) -> LbResult<(File, Uuid)> {
        let mut name = NameComponents::from(&source.name);
        name.next_in_children(self.get_children(parent).await?);
        let name = name.to_name();

        // A link is an alias, not a useful duplicate: copy the target's data
        // into a new document/folder instead. Duplicate links are also invalid
        // at the model layer.
        let source = match source.file_type {
            FileType::Link { target } => self.get_file_by_id(target).await?,
            _ => source,
        };
        let duplicate = self
            .create_file(
                &name,
                parent,
                match source.file_type {
                    FileType::Document => FileType::Document,
                    FileType::Folder => FileType::Folder,
                    FileType::Link { .. } => unreachable!("links are resolved above"),
                },
            )
            .await?;

        if source.is_document() {
            self.write_document(duplicate.id, &self.read_document(source.id, false).await?)
                .await?;
        }

        Ok((duplicate, source.id))
    }

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

        self.events.meta_changed(Actor::User(None));
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

        self.events.meta_changed(Actor::User(None));

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

        self.events.meta_changed(Actor::User(None));

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

        self.events.meta_changed(Actor::User(None));

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

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn get_file_link_url(&self, id: Uuid) -> LbResult<String> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        // Ensure file exists
        let id = tree.find(&id)?.id();

        let account = self.get_account()?;
        let link_url = match account.api_url.as_str() {
            // Use a more user-friendly link for prod API - both route to the same place
            "https://api.prod.lockbook.net" => "https://app.lockbook.net",
            other => other,
        };

        Ok(format!("{}/open/{}", link_url, id))
    }

    pub async fn local_changes(&self) -> Vec<Uuid> {
        let tx = self.ro_tx().await;
        let db = tx.db();
        db.local_metadata.get().keys().copied().collect()
    }
}
