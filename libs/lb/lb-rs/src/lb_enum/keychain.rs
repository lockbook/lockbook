impl Lb {
    pub async fn get_account(&self) -> LbResult<Account>{
        match self {
            Lb::Direct(inner) => {
                let acct_ref: &Account = inner.get_account()?;
                Ok(acct_ref.clone())
            }
            Lb::Network(proxy) => {
                proxy.get_account().await
            }
        }
    }
}

use crate::{model::account::Account, Lb, LbResult};