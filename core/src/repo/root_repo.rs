use uuid::Uuid;

use crate::model::state::Config;
use crate::repo::local_storage;
use crate::CoreError;

static ROOT: &[u8; 4] = b"ROOT";

pub fn set(config: &Config, root: Uuid) -> Result<(), CoreError> {
    let serialized = root
        .to_simple()
        .encode_lower(&mut Uuid::encode_buffer())
        .to_owned()
        .into_bytes();
    local_storage::write(config, ROOT, ROOT, serialized)
}

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

#[cfg(test)]
mod unit_tests {
    use uuid::Uuid;

    use crate::model::state::temp_config;
    use crate::repo::root_repo;

    #[test]
    fn get() {
        let config = &temp_config();

        let result = root_repo::get(config);

        assert!(result.is_err());
    }

    #[test]
    fn maybe_get() {
        let config = &temp_config();

        let result = root_repo::maybe_get(config).unwrap();

        assert_eq!(result, None);
    }

    #[test]
    fn set_get() {
        let config = &temp_config();

        let id = Uuid::new_v4();
        root_repo::set(config, id).unwrap();
        let result = root_repo::get(config).unwrap();

        assert_eq!(result, id);
    }
}
