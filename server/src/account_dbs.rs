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
}
