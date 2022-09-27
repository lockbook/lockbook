use hmdb::log::Reader;
use lockbook_core::service::api_service::no_network::InProcess;
use lockbook_core::service::api_service::*;
use lockbook_server_lib::billing::google_play_client::get_google_play_client;
use lockbook_server_lib::config::*;
use lockbook_server_lib::schema::v2;
use lockbook_server_lib::{stripe, ServerState};
use lockbook_shared::account::Account;
use lockbook_shared::api::*;
use lockbook_shared::file_metadata::FileMetadata;
use lockbook_shared::pubkey;
use std::path::PathBuf;
use std::time::Duration;
use test_utils::test_config;
use test_utils::*;
use tokio::runtime::Runtime;

#[test]
fn new_account_test() {
    let core_config = test_config();
    let server_config = Config {
        server: ServerConfig::from_env_vars(),
        index_db: IndexDbConf {
            db_location: core_config.writeable_path.clone(),
            time_between_compacts: Duration::from_secs(100),
        },
        files: FilesConfig { path: PathBuf::from(core_config.writeable_path) },
        metrics: MetricsConfig::from_env_vars(),
        billing: BillingConfig::from_env_vars(),
        admin: AdminConfig::from_env_vars(),
        features: FeatureFlags::from_env_vars(),
    };

    let stripe_client = stripe::Client::new(&server_config.billing.stripe.stripe_secret);
    let runtime = Runtime::new().unwrap();
    let google_play_client =
        runtime.block_on(get_google_play_client(&server_config.billing.google.service_account_key));

    let index_db =
        v2::Server::init(&server_config.index_db.db_location).expect("Failed to load index_db");

    let client = InProcess {
        server_state: ServerState {
            config: server_config,
            index_db,
            stripe_client,
            google_play_client,
        },
        runtime,
    };

    let account = Account {
        username: "invalid username!".to_string(),
        api_url: "not used!".to_string(),
        private_key: pubkey::generate_key(),
    };

    let root = FileMetadata::create_root(&account)
        .unwrap()
        .sign(&account)
        .unwrap();
    let res = client.request(&account, NewAccountRequest::new(&account, &root));
    assert_matches!(
        res,
        Err(ApiError::<NewAccountError>::Endpoint(NewAccountError::InvalidUsername))
    );
}
