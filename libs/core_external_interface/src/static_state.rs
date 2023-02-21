use lazy_static::lazy_static;
use lockbook_core::{unexpected_only, Config, Core, UnexpectedError};
use std::sync::{Arc, RwLock};

lazy_static! {
    static ref STATE: Arc<RwLock<Option<Core>>> = Arc::new(RwLock::new(None));
}

pub fn init(config: &Config) -> Result<(), UnexpectedError> {
    let core = Core::init(config)?;
    STATE
        .write()
        .map_err(|err| unexpected_only!("{:#?}", err))?
        .replace(core);
    Ok(())
}

pub fn get() -> Result<Core, UnexpectedError> {
    STATE
        .read()
        .map_err(|err| unexpected_only!("{:#?}", err))?
        .clone()
        .ok_or_else(|| unexpected_only!("Did not initialize core prior to using it!"))
}
