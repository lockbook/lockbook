impl Lb {
    pub async fn suggested_docs(&self, settings: RankingWeights) -> LbResult<Vec<Uuid>> {
        match self {
            Lb::Direct(inner) => {
                inner.suggested_docs(settings).await
            }
            Lb::Network(proxy) => {
                proxy.suggested_docs(settings).await
            }
        }
    }
}

use uuid::Uuid;
use crate::service::activity::RankingWeights;
use crate::{Lb, LbResult};