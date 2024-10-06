use std::sync::Arc;

use tokio::runtime::Runtime;

use crate::model::{
    account::Account,
    core_config::Config,
    errors::LbResult,
};


#[derive(Clone)]
pub struct Lb {
    lb: crate::Lb,
    rt: Arc<Runtime>,
}

impl Lb {
    pub fn init(config: Config) -> LbResult<Self> {
        let rt = Arc::new(Runtime::new().unwrap());
        let lb = rt.block_on(crate::Lb::init(config))?;
        Ok(Self { rt, lb })
    }

    pub fn create_account(
        &self, username: &str, api_url: &str, welcome_doc: bool,
    ) -> LbResult<Account> {
        self.rt
            .block_on(self.lb.create_account(username, api_url, welcome_doc))
    }
}
