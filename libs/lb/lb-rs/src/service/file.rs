use crate::Lb;
use crate::io::docs::LocalDocuments;
use crate::io::server::Server;
use crate::model::access_info::UserAccessMode;
use crate::model::errors::{LbErrKind, LbResult};
use crate::model::file::File;
use crate::model::file_like::FileLike;
use crate::model::file_metadata::{FileType, Owner};
use crate::model::filename::{MAX_FILENAME_LENGTH, NameComponents};
use crate::model::lazy::LazyTree;
use crate::model::signed_meta::SignedMeta;
use crate::model::tree_like::TreeLike;
use crate::service::events::Actor;
use std::iter;
use uuid::Uuid;

impl<S: Server, D: LocalDocuments> Lb<S, D> {
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
            self.entropy.id(),
            self.entropy.key(),
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

    /// Creates `document`'s attachments folder, beside it and named for it: `plan.md`'s is
    /// `plan-attachments`. Refused unless this device's [Config::attachments_folders] is on, and
    /// when the document already has one.
    ///
    /// [Config::attachments_folders]: crate::model::core_config::Config::attachments_folders
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn create_attachments_folder(&self, document: &Uuid) -> LbResult<File> {
        if !self.config.attachments_folders {
            return Err(LbErrKind::AttachmentsFoldersOff.into());
        }
        let mut tx = self.begin_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata)
            .to_staged(&mut db.local_metadata)
            .to_lazy();

        let id = tree.create_attachments(
            self.entropy.id(),
            self.entropy.key(),
            document,
            &self.keychain,
        )?;
        let ui_file = tree.decrypt(&self.keychain, &id, &db.pub_key_lookup)?;

        tx.end();

        self.events.meta_changed(Actor::User(None));
        Ok(ui_file)
    }

    /// `document`'s attachments folder, if it has one that isn't deleted.
    #[instrument(level = "debug", skip(self), err(Debug))]
    /// Every attachments folder and everything in it that isn't deleted, including those
    /// [Self::list_metadatas] leaves out, for an app that hides them itself but still finds images
    /// and plans links through them.
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn list_attachments(&self) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let mut ids = Vec::new();
        for id in tree.ids() {
            if tree.in_attachments(&id)?
                && !tree.find(&id)?.is_link()
                && !tree.calculate_deleted(&id)?
            {
                ids.push(id);
            }
        }

        tree.decrypt_all(&self.keychain, ids.into_iter(), &db.pub_key_lookup, false)
    }

    pub async fn get_attachments_folder(&self, document: &Uuid) -> LbResult<Option<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        tree.find(document)?;
        let Some(id) = tree.attachments_folder(document)? else {
            return Ok(None);
        };
        Ok(Some(tree.decrypt(&self.keychain, &id, &db.pub_key_lookup)?))
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
    pub async fn duplicate_file(&self, id: &Uuid) -> LbResult<File> {
        let mut source = self.get_file_by_id(*id).await?;
        if let FileType::Link { target } = source.file_type {
            source = self.get_file_by_id(target).await?;
        }
        if !source.is_document() {
            return Err(LbErrKind::FileNotDocument.into());
        }

        let content = self.read_document(source.id, false).await?;

        let name = NameComponents::from(&source.name)
            .free_copy_in(&self.get_children(&source.parent).await?);

        let new_file = self
            .create_file(&name, &source.parent, FileType::Document)
            .await?;
        self.write_document(new_file.id, &content).await?;

        Ok(new_file)
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

        let root_id = db.root.as_ref().ok_or(LbErrKind::RootNonexistent)?;

        let root = tree.decrypt(&self.keychain, root_id, &db.pub_key_lookup)?;

        Ok(root)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn list_metadatas(&self) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let ids = self.listed(&tree, tree.ids())?;

        tree.decrypt_all(&self.keychain, ids.into_iter(), &db.pub_key_lookup, true)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn get_children(&self, id: &Uuid) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let ids = tree.children_using_links(id)?;
        let ids = self.listed(&tree, ids)?;

        tree.decrypt_all(&self.keychain, ids.into_iter(), &db.pub_key_lookup, true)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn get_and_get_children_recursively(&self, id: &Uuid) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let descendants = tree.descendants_using_links(id)?;
        let descendants = self.listed(&tree, descendants)?;

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

        Ok(format!("{}/open/{}", link_url.trim_end_matches('/'), id))
    }

    /// `ids` without attachments folders and their contents, which are listed only on a device
    /// configured to list them.
    pub(crate) fn listed<T: TreeLike<F = SignedMeta>>(
        &self, tree: &LazyTree<T>, ids: impl IntoIterator<Item = Uuid>,
    ) -> LbResult<Vec<Uuid>> {
        let mut listed = Vec::new();
        for id in ids {
            if self.config.list_attachments || !tree.in_attachments(&id)? {
                listed.push(id);
            }
        }
        Ok(listed)
    }

    pub async fn local_changes(&self) -> Vec<Uuid> {
        let tx = self.ro_tx().await;
        let db = tx.db();
        db.local_metadata
            .iter()
            .map(|(key, _)| key)
            .copied()
            .collect()
    }
}
