use lazy_static::lazy_static;
use lockbook_core::{unexpected_only, Config, UnexpectedError};
use std::sync::{Arc, RwLock};

use crate::FfiCore;

lazy_static! {
    static ref STATE: Arc<RwLock<Option<FfiCore>>> = Arc::new(RwLock::new(None));
}

pub fn init(config: &Config) -> Result<(), UnexpectedError> {
    let core = FfiCore::init(config)?;
    STATE
        .write()
        .map_err(|err| unexpected_only!("{:#?}", err))?
        .replace(core);
    Ok(())
}

pub fn get() -> Result<FfiCore, UnexpectedError> {
    STATE
        .read()
        .map_err(|err| unexpected_only!("{:#?}", err))?
        .clone()
        .ok_or_else(|| unexpected_only!("Did not initialize core prior to using it!"))
}
