impl Lb {
    pub async fn subscribe(&self) -> Receiver<Event>{
        match self {
            Lb::Direct(inner) => {
                inner.subscribe()
            }
            Lb::Network(proxy) => {
                proxy.subscribe().await
            }
        }
    }
}

use tokio::sync::broadcast::Receiver;
use crate::{service::events::Event, Lb};