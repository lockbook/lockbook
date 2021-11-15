use std::collections::HashMap;

use uuid::Uuid;

use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::drawing::{ColorAlias, ColorRGB, Drawing};
use lockbook_models::file_metadata::FileMetadata;

use crate::model::client_conversion::{generate_client_file_metadata, ClientFileMetadata};
use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::pure_functions::files;
use crate::repo::account_repo;
use crate::service::drawing_service::SupportedImageFormats;
use crate::service::file_service;
use crate::service::{drawing_service, file_encryption_service};
use crate::CoreError;

pub fn get_and_get_children_recursively_helper(
    config: &Config,
    id: Uuid,
) -> Result<Vec<FileMetadata>, CoreError> {
    let files = file_service::get_all_not_deleted_metadata(config, RepoSource::Local)?;
    let files = files::filter_not_deleted(&files);
    let file_and_descendants = files::find_with_descendants(&files, id)?;

    // convert from decryptedfilemetadata to filemetadata because that's what this function needs to return for some reason
    let account = account_repo::get(config)?;
    let encrypted_files = file_encryption_service::encrypt_metadata(&account, &files)?;
    let mut result = Vec::new();
    for file in file_and_descendants {
        let encrypted_file = encrypted_files
            .iter()
            .find(|f| f.id == file.id)
            .ok_or_else(|| {
                CoreError::Unexpected(String::from(
                    "get_and_get_children_recursively: encrypted file not found",
                ))
            })?;
        result.push(encrypted_file.clone());
    }
    Ok(result)
}

pub fn delete_file_helper(config: &Config, id: Uuid) -> Result<(), CoreError> {
    let files = file_service::get_all_not_deleted_metadata(config, RepoSource::Local)?;
    let file = files::apply_delete(&files, id)?;
    file_service::insert_metadatum(config, RepoSource::Local, &file)
}

pub fn read_document_helper(config: &Config, id: Uuid) -> Result<DecryptedDocument, CoreError> {
    let all_metadata = file_service::get_all_metadata(config, RepoSource::Local)?;
    file_service::get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)
}

pub fn save_document_to_disk_helper(
    config: &Config,
    id: Uuid,
    location: String,
) -> Result<(), CoreError> {
    let all_metadata = file_service::get_all_metadata(config, RepoSource::Local)?;
    let document =
        file_service::get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)?;
    files::save_document_to_disk(&document, location)
}

pub fn rename_file_helper(config: &Config, id: Uuid, new_name: &str) -> Result<(), CoreError> {
    let files = file_service::get_all_not_deleted_metadata(config, RepoSource::Local)?;
    let files = files::filter_not_deleted(&files);
    let file = files::apply_rename(&files, id, new_name)?;
    file_service::insert_metadatum(config, RepoSource::Local, &file)
}

pub fn move_file_helper(config: &Config, id: Uuid, new_parent: Uuid) -> Result<(), CoreError> {
    let files = file_service::get_all_not_deleted_metadata(config, RepoSource::Local)?;
    let files = files::filter_not_deleted(&files);
    let file = files::apply_move(&files, id, new_parent)?;
    file_service::insert_metadatum(config, RepoSource::Local, &file)
}

pub fn get_drawing_helper(config: &Config, id: Uuid) -> Result<Drawing, CoreError> {
    let all_metadata = file_service::get_all_metadata(config, RepoSource::Local)?;
    let drawing_bytes =
        file_service::get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)?;
    drawing_service::parse_drawing(&drawing_bytes)
}

pub fn save_drawing_helper(
    config: &Config,
    id: Uuid,
    drawing_bytes: &[u8],
) -> Result<(), CoreError> {
    drawing_service::parse_drawing(drawing_bytes)?; // validate drawing
    let metadata = file_service::get_not_deleted_metadata(config, RepoSource::Local, id)?;
    file_service::insert_document(config, RepoSource::Local, &metadata, drawing_bytes)
}

pub fn export_drawing_helper(
    config: &Config,
    id: Uuid,
    format: SupportedImageFormats,
    render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
) -> Result<Vec<u8>, CoreError> {
    let all_metadata = file_service::get_all_metadata(config, RepoSource::Local)?;
    let drawing_bytes =
        file_service::get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)?;
    drawing_service::export_drawing(&drawing_bytes, format, render_theme)
}

pub fn export_drawing_to_disk_helper(
    config: &Config,
    id: Uuid,
    format: SupportedImageFormats,
    render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
    location: String,
) -> Result<(), CoreError> {
    let all_metadata = file_service::get_all_metadata(config, RepoSource::Local)?;
    let drawing_bytes =
        file_service::get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)?;
    let exported_drawing_bytes =
        drawing_service::export_drawing(&drawing_bytes, format, render_theme)?;
    files::save_document_to_disk(&exported_drawing_bytes, location)
}
