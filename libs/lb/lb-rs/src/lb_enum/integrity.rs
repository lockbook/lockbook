impl Lb {
    pub async fn test_repo_integrity(&self) -> LbResult<Vec<Warning>>{
        match self {
            Lb::Direct(inner) => {
                inner.test_repo_integrity().await
            }
            Lb::Network(proxy) => {
                proxy.test_repo_integrity().await
            }
        }
    }
}

use crate::{model::errors::Warning, Lb, LbResult};