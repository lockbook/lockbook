impl Lb {
    pub async fn debug_info(&self, os_info: String) -> LbResult<String>{
        match self {
            Lb::Direct(inner) => {
                inner.debug_info(os_info).await
            }
            Lb::Network(proxy) => {
                proxy.debug_info(os_info).await
            }
        }
    }
}

use crate::{Lb, LbResult};