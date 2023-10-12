use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

use lockbook_shared::document_repo::DocumentService;
use uuid::Uuid;

use lockbook_shared::drawing::{ColorAlias, ColorRGB, Drawing};

use crate::model::drawing;
use crate::model::drawing::SupportedImageFormats;
use crate::Requester;
use crate::{CoreState, LbError, LbResult};

impl<Client: Requester, Docs: DocumentService> CoreState<Client, Docs> {
    pub(crate) fn get_drawing(&mut self, id: Uuid) -> LbResult<Drawing> {
        let doc = self.read_document(id)?;
        drawing::parse_drawing(&doc)
    }

    pub(crate) fn save_drawing(&mut self, id: Uuid, d: &Drawing) -> LbResult<()> {
        drawing::validate(d)?;
        let doc = serde_json::to_vec(d)?;
        self.write_document(id, &doc)
    }

    pub(crate) fn export_drawing(
        &mut self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
    ) -> LbResult<Vec<u8>> {
        let doc = self.read_document(id)?;
        drawing::export_drawing(&doc, format, render_theme)
    }

    pub(crate) fn export_drawing_to_disk(
        &mut self, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>, location: &str,
    ) -> LbResult<()> {
        let doc = self.read_document(id)?;
        let exported_doc = drawing::export_drawing(&doc, format, render_theme)?;
        Self::save_document_to_disk(&exported_doc, location.to_string())
    }

    fn save_document_to_disk(document: &[u8], location: String) -> LbResult<()> {
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(Path::new(&location))
            .map_err(LbError::from)?
            .write_all(document)
            .map_err(LbError::from)?;
        Ok(())
    }
}
