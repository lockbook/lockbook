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
use std::collections::HashMap;
use std::iter;
use uuid::Uuid;

/// The attachments folders [Lb::list_attachments] finds, with what's in them.
#[derive(Debug, Clone, Default)]
pub struct Attachments {
    /// Every attachments folder and everything inside
    pub files: Vec<File>,
    /// Note id -> its attachments folder id
    pub folders: HashMap<Uuid, Uuid>,
}

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
    /// `plan-attachments`. The folder keeps that name when the document is renamed. Refused as
    /// [LbErrKind::InsufficientPermission] while this device doesn't create them
    /// ([Lb::attachments_folders]), and refused when the document already has one.
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn create_attachments_folder(&self, document: &Uuid) -> LbResult<File> {
        if !self.attachments_folders {
            return Err(LbErrKind::InsufficientPermission.into());
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

    /// Every attachments folder and everything in it that isn't deleted, including those
    /// [Self::list_metadatas] leaves out, for an app that hides them itself but still finds images
    /// and plans links through them.
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn list_attachments(&self) -> LbResult<Attachments> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        let mut ids = Vec::new();
        let mut folders = HashMap::new();
        for id in tree.ids() {
            if tree.in_attachments(&id)?
                && !tree.find(&id)?.is_link()
                && !tree.calculate_deleted(&id)?
            {
                ids.push(id);
                if let Some(of) = tree.find(&id)?.attachment_of() {
                    folders.entry(of.id).or_insert(id);
                }
            }
        }

        let files = tree.decrypt_all(&self.keychain, ids.into_iter(), &db.pub_key_lookup, false)?;
        Ok(Attachments { files, folders })
    }

    /// `document`'s attachments folder, if it has one that isn't deleted.
    #[instrument(level = "debug", skip(self), err(Debug))]
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

    /// The document `folder` is the attachments folder of, if it is one. Such a folder moves only
    /// with its document and is never renamed, so a host that offers rename and move per file can leave
    /// them off it.
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn attachments_of(&self, folder: &Uuid) -> LbResult<Option<Uuid>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();
        Ok(tree.find(folder)?.attachment_of().map(|of| of.id))
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
            .free_copy_in(&self.all_children(&source.parent).await?);

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

    /// All files as this client lists them: attachments hidden for desktop and mobile, shown for cli
    /// and files integrations
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn list_metadatas(&self) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let mut ids = tree.ids();
        if !self.config.list_attachments {
            ids = Self::omit_attachments(&mut tree, ids)?;
        }

        tree.decrypt_all(&self.keychain, ids.into_iter(), &db.pub_key_lookup, true)
    }

    /// Every file, attachments folders and their contents included, for lb's own walks.
    pub(crate) async fn all_metadatas(&self) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let ids = tree.ids();

        tree.decrypt_all(&self.keychain, ids.into_iter(), &db.pub_key_lookup, true)
    }

    /// A folder's children as this client lists them: attachments hidden for desktop and mobile,
    /// shown for cli and files integrations
    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn get_children(&self, id: &Uuid) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let mut ids = tree.children_using_links(id)?;
        if !self.config.list_attachments {
            ids = Self::omit_attachments(&mut tree, ids)?;
        }

        tree.decrypt_all(&self.keychain, ids.into_iter(), &db.pub_key_lookup, true)
    }

    /// All of `id`'s children that aren't deleted, including the ones listings leave out, for lb's
    /// own walks and name checks.
    pub(crate) async fn all_children(&self, id: &Uuid) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let mut ids = Vec::new();
        for child in tree.children_using_links(id)? {
            if !tree.calculate_deleted(&child)? {
                ids.push(child);
            }
        }

        tree.decrypt_all(&self.keychain, ids.into_iter(), &db.pub_key_lookup, false)
    }

    #[instrument(level = "debug", skip(self), err(Debug))]
    pub async fn get_and_get_children_recursively(&self, id: &Uuid) -> LbResult<Vec<File>> {
        let tx = self.ro_tx().await;
        let db = tx.db();

        let mut tree = (&db.base_metadata).to_staged(&db.local_metadata).to_lazy();

        let mut descendants = tree.descendants_using_links(id)?;
        if !self.config.list_attachments {
            descendants = Self::omit_attachments(&mut tree, descendants)?;
        }

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

    /// `ids` without attachments folders and their contents, for a listing on a client whose
    /// config doesn't list them.
    pub(crate) fn omit_attachments<T, Ids>(tree: &mut LazyTree<T>, ids: Ids) -> LbResult<Ids>
    where
        T: TreeLike<F = SignedMeta>,
        Ids: IntoIterator<Item = Uuid> + FromIterator<Uuid>,
    {
        let mut kept = Vec::new();
        for id in ids {
            if !tree.in_attachments(&id)? {
                kept.push(id);
            }
        }
        Ok(kept.into_iter().collect())
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
