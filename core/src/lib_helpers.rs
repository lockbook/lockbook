use crate::model::client_conversion::{
    generate_client_file_metadata, generate_client_work_calculated, ClientFileMetadata,
    ClientWorkCalculated,
};
use crate::model::repo::RepoSource;
use crate::model::state::Config;
use crate::repo::account_repo;
use crate::repo::file_repo;
use crate::service::drawing_service::SupportedImageFormats;
use crate::service::{
    drawing_service, file_encryption_service, file_service, path_service, sync_service,
};
use crate::{loggers, unexpected, utils, CoreError, Error, LOG_FILE};
use lockbook_models::crypto::DecryptedDocument;
use lockbook_models::drawing::{ColorAlias, ColorRGB, Drawing};
use lockbook_models::file_metadata::{FileMetadata, FileType};
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::str::FromStr;
use uuid::Uuid;

pub fn init_logger_helper(log_path: &Path) -> Result<(), Error<()>> {
    let print_colors = env::var("LOG_NO_COLOR").is_err();
    let lockbook_log_level = env::var("LOG_LEVEL")
        .ok()
        .and_then(|s| log::LevelFilter::from_str(s.as_str()).ok())
        .unwrap_or(log::LevelFilter::Debug);

    loggers::init(log_path, LOG_FILE.to_string(), print_colors)
        .map_err(|err| unexpected!("IO Error: {:#?}", err))?
        .level(log::LevelFilter::Warn)
        .level_for("lockbook_core", lockbook_log_level)
        .apply()
        .map_err(|err| unexpected!("{:#?}", err))?;
    info!("Logger initialized! Path: {:?}", log_path);
    Ok(())
}

pub fn create_file_at_path_helper(
    config: &Config,
    path_and_name: &str,
) -> Result<ClientFileMetadata, CoreError> {
    let file_metadata = path_service::create_at_path(config, path_and_name)?;
    generate_client_file_metadata(&file_metadata)
}

pub fn write_document(config: &Config, id: Uuid, content: &[u8]) -> Result<(), CoreError> {
    let metadata = file_repo::get_not_deleted_metadata(config, RepoSource::Local, id)?;
    file_repo::insert_document(config, RepoSource::Local, &metadata, content)
}

pub fn create_file(
    config: &Config,
    name: &str,
    parent: Uuid,
    file_type: FileType,
) -> Result<ClientFileMetadata, CoreError> {
    let account = account_repo::get(config)?;
    file_repo::get_not_deleted_metadata(config, RepoSource::Local, parent)?;
    let all_metadata = file_repo::get_all_metadata(config, RepoSource::Local)?;
    let metadata =
        file_service::apply_create(&all_metadata, file_type, parent, name, &account.username)?;
    file_repo::insert_metadatum(config, RepoSource::Local, &metadata)?;
    generate_client_file_metadata(&metadata)
}

pub fn get_root_helper(config: &Config) -> Result<ClientFileMetadata, CoreError> {
    let files = file_repo::get_all_not_deleted_metadata(config, RepoSource::Local)?;
    match utils::maybe_find_root(&files) {
        None => Err(CoreError::RootNonexistent),
        Some(file_metadata) => generate_client_file_metadata(&file_metadata),
    }
}

pub fn get_children_helper(
    config: &Config,
    id: Uuid,
) -> Result<Vec<ClientFileMetadata>, CoreError> {
    let files = file_repo::get_all_not_deleted_metadata(config, RepoSource::Local)?;
    let files = utils::filter_not_deleted(&files);
    let children = utils::find_children(&files, id);
    children
        .iter()
        .map(|c| generate_client_file_metadata(c))
        .collect()
}

pub fn get_and_get_children_recursively_helper(
    config: &Config,
    id: Uuid,
) -> Result<Vec<FileMetadata>, CoreError> {
    let files = file_repo::get_all_not_deleted_metadata(config, RepoSource::Local)?;
    let files = utils::filter_not_deleted(&files);
    let file_and_descendants = utils::find_with_descendants(&files, id)?;

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

pub fn get_file_by_id_helper(config: &Config, id: Uuid) -> Result<ClientFileMetadata, CoreError> {
    let file_metadata = file_repo::get_not_deleted_metadata(config, RepoSource::Local, id)?;
    generate_client_file_metadata(&file_metadata)
}

pub fn get_file_by_path_helper(
    config: &Config,
    path: &str,
) -> Result<ClientFileMetadata, CoreError> {
    let file_metadata = path_service::get_by_path(config, path)?;
    generate_client_file_metadata(&file_metadata)
}

pub fn delete_file_helper(config: &Config, id: Uuid) -> Result<(), CoreError> {
    let files = file_repo::get_all_not_deleted_metadata(config, RepoSource::Local)?;
    let file = file_service::apply_delete(&files, id)?;
    file_repo::insert_metadatum(config, RepoSource::Local, &file)
}

pub fn read_document_helper(config: &Config, id: Uuid) -> Result<DecryptedDocument, CoreError> {
    let all_metadata = file_repo::get_all_metadata(config, RepoSource::Local)?;
    file_repo::get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)
}

pub fn save_document_to_disk_helper(
    config: &Config,
    id: Uuid,
    location: String,
) -> Result<(), CoreError> {
    let all_metadata = file_repo::get_all_metadata(config, RepoSource::Local)?;
    let document =
        file_repo::get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)?;
    file_service::save_document_to_disk(&document, location)
}

pub fn list_metadatas_helper(config: &Config) -> Result<Vec<ClientFileMetadata>, CoreError> {
    let metas = file_repo::get_all_not_deleted_metadata(config, RepoSource::Local)?;
    let mut client_metas = vec![];
    for meta in metas {
        client_metas.push(generate_client_file_metadata(&meta)?);
    }
    Ok(client_metas)
}

pub fn rename_file_helper(config: &Config, id: Uuid, new_name: &str) -> Result<(), CoreError> {
    let files = file_repo::get_all_not_deleted_metadata(config, RepoSource::Local)?;
    let files = utils::filter_not_deleted(&files);
    let file = file_service::apply_rename(&files, id, new_name)?;
    file_repo::insert_metadatum(config, RepoSource::Local, &file)
}

pub fn move_file_helper(config: &Config, id: Uuid, new_parent: Uuid) -> Result<(), CoreError> {
    let files = file_repo::get_all_not_deleted_metadata(config, RepoSource::Local)?;
    let files = utils::filter_not_deleted(&files);
    let file = file_service::apply_move(&files, id, new_parent)?;
    file_repo::insert_metadatum(config, RepoSource::Local, &file)
}

pub fn calculate_work_helper(config: &Config) -> Result<ClientWorkCalculated, CoreError> {
    let work_calculated = sync_service::calculate_work(config)?;
    generate_client_work_calculated(&work_calculated)
}

pub fn get_drawing_helper(config: &Config, id: Uuid) -> Result<Drawing, CoreError> {
    let all_metadata = file_repo::get_all_metadata(config, RepoSource::Local)?;
    let drawing_bytes =
        file_repo::get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)?;
    drawing_service::parse_drawing(&drawing_bytes)
}

pub fn save_drawing_helper(
    config: &Config,
    id: Uuid,
    drawing_bytes: &[u8],
) -> Result<(), CoreError> {
    drawing_service::parse_drawing(drawing_bytes)?; // validate drawing
    let metadata = file_repo::get_not_deleted_metadata(config, RepoSource::Local, id)?;
    file_repo::insert_document(config, RepoSource::Local, &metadata, drawing_bytes)
}

pub fn export_drawing_helper(
    config: &Config,
    id: Uuid,
    format: SupportedImageFormats,
    render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
) -> Result<Vec<u8>, CoreError> {
    let all_metadata = file_repo::get_all_metadata(config, RepoSource::Local)?;
    let drawing_bytes =
        file_repo::get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)?;
    drawing_service::export_drawing(&drawing_bytes, format, render_theme)
}

pub fn export_drawing_to_disk_helper(
    config: &Config,
    id: Uuid,
    format: SupportedImageFormats,
    render_theme: Option<HashMap<ColorAlias, ColorRGB>>,
    location: String,
) -> Result<(), CoreError> {
    let all_metadata = file_repo::get_all_metadata(config, RepoSource::Local)?;
    let drawing_bytes =
        file_repo::get_not_deleted_document(config, RepoSource::Local, &all_metadata, id)?;
    let exported_drawing_bytes =
        drawing_service::export_drawing(&drawing_bytes, format, render_theme)?;
    file_service::save_document_to_disk(&exported_drawing_bytes, location)
}
