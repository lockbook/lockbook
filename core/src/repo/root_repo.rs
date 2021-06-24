use crate::model::state::Config;
use crate::repo::{file_repo, local_storage};
use crate::CoreError;
use lockbook_models::file_metadata::FileMetadata;
use uuid::Uuid;

pub static ROOT: &[u8; 4] = b"ROOT";

pub fn get(config: &Config) -> Result<Option<FileMetadata>, CoreError> {
    let maybe_value: Option<Vec<u8>> = local_storage::read(config, ROOT, ROOT)?;
    match maybe_value {
        None => Ok(None),
        Some(value) => {
            match String::from_utf8(value.clone()) {
                Ok(id) => match Uuid::parse_str(&id) {
                    Ok(uuid) => file_repo::maybe_get_metadata(&config, uuid).map(
                        |maybe_root_and_repo_state| maybe_root_and_repo_state.map(|(root, _)| root),
                    ),
                    Err(err) => {
                        error!("Failed to parse {:?} into a UUID. Error: {:?}", id, err);
                        Ok(None)
                    }
                },
                Err(err) => {
                    error!("Failed parsing {:?} into a UUID. Error: {:?}", &value, err);
                    Ok(None)
                }
            }
        }
    }
}
