use db_rs::Db;
use lb_rs::model::file_metadata::Owner;
use std::{collections::HashMap, fmt::Debug, sync::Arc};
use tokio::sync::RwLock;

use crate::{
    billing::{
        app_store_client::AppStoreClient, google_play_client::GooglePlayClient,
        stripe_client::StripeClient,
    },
    document_service::DocumentService,
    schema::AccountV1,
    server_tree::ServerTreeV2,
    ServerError, ServerState,
};

pub type AccountDb = Arc<RwLock<AccountV1>>;
pub type AccountDbs = Arc<RwLock<HashMap<Owner, AccountDb>>>;

impl<S, A, G, D> ServerState<S, A, G, D>
where
    S: StripeClient,
    A: AppStoreClient,
    G: GooglePlayClient,
    D: DocumentService,
{
    pub async fn new_db<T: Debug>(&self, owner: Owner) -> Result<AccountDb, ServerError<T>> {
        let account_db =
            AccountV1::init(db_rs::Config::in_folder(&self.config.index_db.db_location))?;
        let account_db = Arc::new(RwLock::new(account_db));

        let mut account_dbs = self.account_dbs.write().await;
        account_dbs.insert(owner, account_db.clone());

        Ok(account_db)
    }

    pub async fn get_tree_old<T: Debug>(
        &self, owner: Owner,
    ) -> Result<ServerTreeV2, ServerError<T>> {
        let owner_dbs = self.account_dbs.read().await;
        let mut owners: Vec<Owner> = owner_dbs
            .get(&owner)
            .unwrap()
            .read()
            .await
            .shared_files
            .get()
            .iter()
            .map(|(_id, owner)| *owner)
            .collect();

        owners.push(owner);
        owners.sort_unstable_by_key(|owner| owner.0.serialize());
        owners.dedup();

        // there is a gap in consistency here and there doesn't need to be
        let mut trees = vec![];
        for owner in owners {
            let db = owner_dbs.get(&owner).unwrap().clone();
            let db = db.write_owned().await;
            trees.push((owner, db));
        }

        Ok(ServerTreeV2 { owner, trees })
    }
}
