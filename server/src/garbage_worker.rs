use std::time::Duration;
use tracing::{debug, error, info};

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
                tokio::time::sleep(Duration::from_secs(60)).await;
                info!("garbage collecting");
                bg_self.garbage_collect().await;
                info!("garbage collected");
            }
        });
    }

    pub async fn garbage_collect(&self) {
        let mut db = self.index_db.lock().await;
        let files = db.scheduled_file_cleanups.get();
        let mut cleaned = 0;
        let mut skipped = 0;
        let mut remove = vec![];
        for ((id, hmac), time) in files {
            if get_time().0 - time > 1000 * 60 * 5 {
                cleaned += 1;
                if let Err(e) = self.document_service.delete::<()>(id, hmac).await {
                    error!(
                        "failed to garbage collect {:?} {e:?}",
                        self.document_service.get_path(id, hmac)
                    );
                } else {
                    remove.push((*id, *hmac));
                }
                debug!("garbage collected: {:?}", self.document_service.get_path(id, hmac));
            } else {
                skipped += 1;
            }
        }

        for (id, hmac) in remove {
            db.scheduled_file_cleanups.remove(&(id, hmac)).unwrap();
        }

        info!("cleaned {cleaned}, skipped {skipped}");
    }
}
