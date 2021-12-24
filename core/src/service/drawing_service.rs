use crate::model::errors::CoreError;
use std::collections::HashMap;

use uuid::Uuid;

use lockbook_models::drawing::{ColorAlias, ColorRGB, Drawing};

use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::pure_functions::{drawing, files};

use crate::pure_functions::drawing::SupportedImageFormats;
use crate::service::file_service;

pub fn save_drawing(config: &Config, id: Uuid, drawing_bytes: &[u8]) -> Result<(), CoreError> {
    info!("writing (drawing) {} bytes to {}", drawing_bytes.len(), id);
    drawing::parse_drawing(drawing_bytes)?; // validate drawing
    let metadata = file_service::get_not_deleted_metadata(config, RepoSource::Local, id)?;
    file_service::insert_document(config, RepoSource::Local, &metadata, drawing_bytes)
}

pub fn export_drawing(
    config: &Config,
    id: Uuid,
    format: SupportedImageFormats,
    render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
) -> Result<Vec<u8>, CoreError> {
    info!("exporting drawing {} as {:?}", id, format);
    let all_metadata = file_service::get_all_metadata(config, RepoSource::Local)?;
    let drawing_bytes =
        file_service::get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)?;
    drawing::export_drawing(&drawing_bytes, format, render_theme)
}

pub fn export_drawing_to_disk(
    config: &Config,
    id: Uuid,
    format: SupportedImageFormats,
    render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
    location: &str,
) -> Result<(), CoreError> {
    info!("exporting drawing {} to {} as {:?}", id, location, format);
    let all_metadata = file_service::get_all_metadata(config, RepoSource::Local)?;
    let drawing_bytes =
        file_service::get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)?;
    let exported_drawing_bytes = drawing::export_drawing(&drawing_bytes, format, render_theme)?;
    files::save_document_to_disk(&exported_drawing_bytes, location.to_string())
}

pub fn get_drawing(config: &Config, id: Uuid) -> Result<Drawing, CoreError> {
    info!("getting drawing: {}", id);
    let all_metadata = file_service::get_all_metadata(config, RepoSource::Local)?;
    let drawing_bytes =
        file_service::get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)?;
    drawing::parse_drawing(&drawing_bytes)
}
