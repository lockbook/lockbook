use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use uuid::Uuid;

use lockbook_shared::drawing::{ColorAlias, ColorRGB, Drawing};
use lockbook_shared::file_metadata::Owner;
use lockbook_shared::lazy::LazyStaged1;
use lockbook_shared::tree_like::TreeLike;
use lockbook_shared::validate;

use crate::model::drawing;
use crate::model::drawing::SupportedImageFormats;
use crate::model::errors::CoreError;
use crate::model::repo::RepoSource;
use crate::repo::document_repo;
use crate::RequestContext;
use crate::{CoreResult, OneKey};

impl RequestContext<'_, '_> {
    pub fn get_drawing(&mut self, id: Uuid) -> CoreResult<Drawing> {
        let mut tree = LazyStaged1::core_tree(
            Owner(self.get_public_key()?),
            &mut self.tx.base_metadata,
            &mut self.tx.local_metadata,
        );
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let doc = document_repo::get(self.config, RepoSource::Local, id)?;

        drawing::parse_drawing(&tree.decrypt_document(&id, &doc, account)?)
    }

    pub fn save_drawing(&mut self, id: Uuid, d: &Drawing) -> CoreResult<()> {
        let mut tree = LazyStaged1::core_tree(
            Owner(self.get_public_key()?),
            &mut self.tx.base_metadata,
            &mut self.tx.local_metadata,
        );
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        if tree.calculate_deleted(&id)? {
            return Err(CoreError::FileNonexistent);
        }

        let drawing_bytes = serde_json::to_vec(d)?;
        let (_, doc) = tree.update_document(&id, &drawing_bytes, account)?;

        document_repo::insert(self.config, RepoSource::Local, id, &doc)
    }

    pub fn export_drawing(
        &mut self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
    ) -> CoreResult<Vec<u8>> {
        let mut tree = LazyStaged1::core_tree(
            Owner(self.get_public_key()?),
            &mut self.tx.base_metadata,
            &mut self.tx.local_metadata,
        );
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        if tree.calculate_deleted(&id)? {
            return Err(CoreError::FileNonexistent);
        }

        let doc = document_repo::get(self.config, RepoSource::Local, id)?;

        drawing::export_drawing(&tree.decrypt_document(&id, &doc, account)?, format, render_theme)
    }

    pub fn export_drawing_to_disk(
        &mut self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>, location: &str,
    ) -> CoreResult<()> {
        let mut tree = LazyStaged1::core_tree(
            Owner(self.get_public_key()?),
            &mut self.tx.base_metadata,
            &mut self.tx.local_metadata,
        );
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        if tree.calculate_deleted(&id)? {
            return Err(CoreError::FileNonexistent);
        }

        let meta = tree.find(&id)?;
        validate::is_document(&meta)?;

        let doc = document_repo::get(self.config, RepoSource::Local, id)?;
        let exported_drawing_bytes = drawing::export_drawing(
            &tree.decrypt_document(&id, &doc, account)?,
            format,
            render_theme,
        )?;

        Self::save_document_to_disk(&exported_drawing_bytes, location.to_string())
    }

    fn save_document_to_disk(document: &[u8], location: String) -> CoreResult<()> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(Path::new(&location))
            .map_err(CoreError::from)?
            .write_all(document)
            .map_err(CoreError::from)?;
        Ok(())
    }
}
