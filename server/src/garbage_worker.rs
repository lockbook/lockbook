use crate::guard::ServerTx;
use std::time::Duration;
use tracing::{debug, error, info};

use lb_rs::model::clock::get_time;

use crate::doc_info_service::retained;
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
        let mut guard = self.index_db.lock().await;
        let mut db = match ServerTx::begin(&mut guard) {
            Ok(tx) => tx,
            Err(error) => {
                error!(?error, "failed to begin garbage collection transaction");
                return;
            }
        };
        let now = get_time().0;
        let mut cleaned = 0;
        let mut skipped = 0;
        let mut remove = vec![];
        let due: Vec<_> = db
            .scheduled_blob_cleanups
            .iter()
            .map(|(key, time)| (*key, *time))
            .collect();
        for (key, time) in due {
            if now - time <= 1000 * 60 * 5 {
                skipped += 1;
                continue;
            }
            if retained(db.doc_info.get(&key.id()), &key) {
                remove.push(key);
                continue;
            }
            cleaned += 1;
            if let Err(e) = self.document_service.delete::<()>(&key).await {
                error!(
                    "failed to garbage collect {:?} {e:?}",
                    self.document_service.get_path(&key)
                );
            } else {
                remove.push(key);
                if let Some(mut info) = db.doc_info.get(&key.id()).cloned() {
                    info.forget_retired(&key);
                    if let Err(error) = db.doc_info.insert(key.id(), info) {
                        error!(?error, "failed to forget a collected document version");
                    }
                }
            }
            debug!("garbage collected: {:?}", self.document_service.get_path(&key));
        }

        for key in remove {
            db.scheduled_blob_cleanups.remove(&key).unwrap();
        }

        if let Err(error) = db.end() {
            error!(?error, "failed to flush garbage collection transaction");
        }

        info!("cleaned {cleaned}, skipped {skipped}");
    }
}
