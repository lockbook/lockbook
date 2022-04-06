use crate::{unexpected_only, Config, LbCore, UnexpectedError};
use lazy_static::lazy_static;
use std::sync::{Arc, RwLock};

lazy_static! {
    static ref STATE: Arc<RwLock<Option<LbCore>>> = Arc::new(RwLock::new(None));
}

pub fn init(config: &Config) -> Result<(), UnexpectedError> {
    let core = LbCore::init(config)?;
    STATE
        .write()
        .map_err(|err| unexpected_only!("{:#?}", err))?
        .replace(core);
    Ok(())
}

pub fn get() -> Result<LbCore, UnexpectedError> {
    STATE
        .read()
        .map_err(|err| unexpected_only!("{:#?}", err))?
        .clone()
        .ok_or_else(|| unexpected_only!("Did not initialize core prior to using it!"))
}
