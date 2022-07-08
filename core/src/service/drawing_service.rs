use std::collections::HashMap;

use uuid::Uuid;

use lockbook_models::drawing::{ColorAlias, ColorRGB, Drawing};

use crate::model::errors::CoreError;
use crate::model::repo::RepoSource;
use crate::pure_functions::drawing::SupportedImageFormats;
use crate::pure_functions::{drawing, files};
use crate::service::file_service;
use crate::{Config, RequestContext};

impl RequestContext<'_, '_> {
    pub fn get_drawing(&mut self, config: &Config, id: Uuid) -> Result<Drawing, CoreError> {
        let all_metadata = self.get_all_metadata(RepoSource::Local)?;
        let drawing_bytes =
            file_service::get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)?;
        drawing::parse_drawing(&drawing_bytes)
    }

    pub fn save_drawing(
        &mut self, config: &Config, id: Uuid, d: &Drawing,
    ) -> Result<(), CoreError> {
        let metadata = self.get_not_deleted_metadata(RepoSource::Local, id)?;
        let drawing_bytes = serde_json::to_vec(d)?;
        self.insert_document(config, RepoSource::Local, &metadata, &drawing_bytes)
    }

    pub fn export_drawing(
        &mut self, config: &Config, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
    ) -> Result<Vec<u8>, CoreError> {
        let all_metadata = self.get_all_metadata(RepoSource::Local)?;
        let drawing_bytes =
            file_service::get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)?;
        drawing::export_drawing(&drawing_bytes, format, render_theme)
    }

    pub fn export_drawing_to_disk(
        &mut self, config: &Config, id: Uuid, format: SupportedImageFormats,
        render_theme: Option<HashMap<ColorAlias, ColorRGB>>, location: &str,
    ) -> Result<(), CoreError> {
        let all_metadata = self.get_all_metadata(RepoSource::Local)?;
        let drawing_bytes =
            file_service::get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)?;
        let exported_drawing_bytes = drawing::export_drawing(&drawing_bytes, format, render_theme)?;
        files::save_document_to_disk(&exported_drawing_bytes, location.to_string())
    }
}
