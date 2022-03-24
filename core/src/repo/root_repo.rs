use uuid::Uuid;

use crate::model::errors::core_err_unexpected;
use crate::model::state::Config;
use crate::repo::local_storage;
use crate::CoreError;

static ROOT: &[u8; 4] = b"ROOT";

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn set(config: &Config, root: Uuid) -> Result<(), CoreError> {
    let serialized = root
        .to_simple()
        .encode_lower(&mut Uuid::encode_buffer())
        .to_owned()
        .into_bytes();
    local_storage::write(config, ROOT, ROOT, serialized)
}

#[instrument(level = "debug", skip(config), err(Debug))]
pub fn maybe_get(config: &Config) -> Result<Option<Uuid>, CoreError> {
    let maybe_value: Option<Vec<u8>> = local_storage::read(config, ROOT, ROOT)?;
    Ok(match maybe_value {
        None => None,
        Some(value) => Some(
            Uuid::parse_str(&String::from_utf8(value).map_err(core_err_unexpected)?)
                .map_err(core_err_unexpected)?,
        ),
    })
}

#[instrument(level = "debug", skip(config), err(Debug))]
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
