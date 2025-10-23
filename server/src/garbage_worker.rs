use std::time::Duration;
use tracing::{error, info};

use lb_rs::model::clock::get_time;

use crate::{
    ServerState,
    billing::{
        app_store_client::AppStoreClient, google_play_client::GooglePlayClient,
        stripe_client::StripeClient,
    },
    document_service::DocumentService,
};

impl<S, A, G, D> ServerState<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    pub fn start_garbage_worker(&self) {
        let bg_self = self.clone();

        tokio::spawn(async move {
            loop {
                info!("garbage collecting");
                bg_self.garbage_collect().await;
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
    }

    pub async fn garbage_collect(&self) {
        let db = self.index_db.lock().await;
        let files = db.scheduled_file_cleanups.get();
        for ((id, hmac), time) in files {
            if get_time().0 - time > 1000 * 60 * 10 {
                if let Err(e) = self.document_service.delete::<()>(id, hmac).await {
                    error!(
                        "failed to garbage collect {:?} {e:?}",
                        self.document_service.get_path(id, hmac)
                    );
                }
            }
        }
    }
}
