use crate::model::state::Config;
use crate::repo::local_storage;
use crate::CoreError;
use uuid::Uuid;

pub static ROOT: &[u8; 4] = b"ROOT";

pub fn maybe_get(config: &Config) -> Result<Option<Uuid>, CoreError> {
    let maybe_value: Option<Vec<u8>> = local_storage::read(config, ROOT, ROOT)?;
    match maybe_value {
        None => Ok(None),
        Some(value) => match String::from_utf8(value.clone()) {
            Ok(id) => match Uuid::parse_str(&id) {
                Ok(id) => Ok(Some(id)),
                Err(err) => {
                    error!("Failed to parse {:?} into a UUID. Error: {:?}", id, err);
                    Ok(None)
                }
            },
            Err(err) => {
                error!("Failed to parse {:?} into a UUID. Error: {:?}", &value, err);
                Ok(None)
            }
        },
    }
}

pub fn get(config: &Config) -> Result<Uuid, CoreError> {
    maybe_get(config).and_then(|f| f.ok_or(CoreError::RootNonexistent))
}
