use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use uuid::Uuid;

use lockbook_shared::drawing::{ColorAlias, ColorRGB, Drawing};
use lockbook_shared::tree_like::Stagable;

use crate::model::drawing;
use crate::model::drawing::SupportedImageFormats;
use crate::model::errors::CoreError;
use crate::{CoreResult, OneKey};
use crate::{RequestContext, Requester};

impl<Client: Requester> RequestContext<'_, '_, Client> {
    pub fn get_drawing(&mut self, id: Uuid) -> CoreResult<Drawing> {
        let tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let doc = tree.read_document(self.config, &id, account)?.1;

        drawing::parse_drawing(&doc)
    }

    pub fn save_drawing(&mut self, id: Uuid, d: &Drawing) -> CoreResult<()> {
        drawing::validate(d)?;

        let tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let drawing_bytes = serde_json::to_vec(d)?;
        tree.write_document(self.config, &id, &drawing_bytes, account)?;
        Ok(())
    }

    pub fn export_drawing(
        &mut self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
    ) -> CoreResult<Vec<u8>> {
        let tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let doc = tree.read_document(self.config, &id, account)?.1;

        drawing::export_drawing(&doc, format, render_theme)
    }

    pub fn export_drawing_to_disk(
        &mut self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>, location: &str,
    ) -> CoreResult<()> {
        let tree = self
            .tx
            .base_metadata
            .stage(&mut self.tx.local_metadata)
            .to_lazy();
        let account = self
            .tx
            .account
            .get(&OneKey {})
            .ok_or(CoreError::AccountNonexistent)?;

        let doc = tree.read_document(self.config, &id, account)?.1;
        let exported_drawing_bytes = drawing::export_drawing(&doc, format, render_theme)?;
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
