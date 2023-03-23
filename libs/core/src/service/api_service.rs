use reqwest::blocking::Client as RequestClient;

use crate::get_code_version;
use lockbook_shared::account::Account;
use lockbook_shared::api::*;
use lockbook_shared::clock::{get_time, Timestamp};
use lockbook_shared::pubkey;

impl<E> From<ErrorWrapper<E>> for ApiError<E> {
    fn from(err: ErrorWrapper<E>) -> Self {
        match err {
            ErrorWrapper::Endpoint(e) => ApiError::Endpoint(e),
            ErrorWrapper::ClientUpdateRequired => ApiError::ClientUpdateRequired,
            ErrorWrapper::InvalidAuth => ApiError::InvalidAuth,
            ErrorWrapper::ExpiredAuth => ApiError::ExpiredAuth,
            ErrorWrapper::InternalError => ApiError::InternalError,
            ErrorWrapper::BadRequest => ApiError::BadRequest,
        }
    }
}

// #[derive(Debug, PartialEq, Eq)]
#[derive(Debug)]
pub enum ApiError<E> {
    Endpoint(E),
    ClientUpdateRequired,
    InvalidAuth,
    ExpiredAuth,
    InternalError,
    BadRequest,
    Sign(lockbook_shared::SharedError),
    Serialize(String),
    SendFailed(String),
    ReceiveFailed(String),
    Deserialize(String),
}

pub trait Requester {
    fn request<T: Request>(
        &self, account: &Account, request: T,
    ) -> Result<T::Response, ApiError<T::Error>>;
}

#[derive(Debug, Clone)]
pub struct Network {
    pub client: RequestClient,
    pub get_code_version: fn() -> &'static str,
    pub get_time: fn() -> Timestamp,
}

impl Default for Network {
    fn default() -> Self {
        Self { client: Default::default(), get_code_version, get_time }
    }
}

impl Requester for Network {
    fn request<T: Request>(
        &self, account: &Account, request: T,
    ) -> Result<T::Response, ApiError<T::Error>> {
        let signed_request =
            pubkey::sign(&account.private_key, request, self.get_time).map_err(ApiError::Sign)?;

        let client_version = String::from((self.get_code_version)());

        let serialized_request = serde_json::to_vec(&RequestWrapper {
            signed_request,
            client_version: client_version.clone(),
        })
        .map_err(|err| ApiError::Serialize(err.to_string()))?;
        let serialized_response = self
            .client
            .request(T::METHOD, format!("{}{}", account.api_url, T::ROUTE).as_str())
            .body(serialized_request)
            .header("Accept-Version", client_version)
            .send()
            .map_err(|err| {
                warn!("Send failed: {:#?}", err);
                ApiError::SendFailed(err.to_string())
            })?
            .bytes()
            .map_err(|err| ApiError::ReceiveFailed(err.to_string()))?;
        let response: Result<T::Response, ErrorWrapper<T::Error>> =
            serde_json::from_slice(&serialized_response)
                .map_err(|err| ApiError::Deserialize(err.to_string()))?;
        response.map_err(ApiError::from)
    }
}

#[cfg(feature = "no-network")]
pub mod no_network {

    use crate::service::api_service::ApiError;
    use crate::{call, CoreLib, CoreState};
    use crate::{CoreDb, Requester};
    use db_rs::Db;
    use lockbook_server_lib::account_service::*;
    use lockbook_server_lib::billing::google_play_client::get_google_play_client;
    use lockbook_server_lib::config::*;
    use lockbook_server_lib::file_service::*;
    use lockbook_server_lib::schema::ServerV4;
    use lockbook_server_lib::{stripe, ServerError, ServerState};
    use lockbook_shared::account::Account;
    use lockbook_shared::api::*;
    use lockbook_shared::core_config::Config;
    use std::any::Any;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::runtime;
    use tokio::runtime::Runtime;

    #[derive(Clone)]
    pub struct InProcess {
        pub config: Config,
        pub internals: Arc<Mutex<InProcessInternals>>,
    }

    pub struct InProcessInternals {
        pub server_state: ServerState,
        pub runtime: Runtime,
    }

    impl InProcess {
        pub fn init(config: Config, admin: AdminConfig) -> Self {
            let runtime = runtime::Builder::new_current_thread().build().unwrap();
            let server_config = lockbook_server_lib::config::Config {
                server: ServerConfig::from_env_vars(),
                index_db: IndexDbConf {
                    db_location: config.writeable_path.clone(),
                    time_between_compacts: Duration::from_secs(0),
                },
                files: FilesConfig { path: PathBuf::from(&config.writeable_path) },
                metrics: MetricsConfig::from_env_vars(),
                billing: BillingConfig::from_env_vars(),
                admin,
                features: FeatureFlags::from_env_vars(),
            };

            let stripe_client = stripe::Client::new(&server_config.billing.stripe.stripe_secret);
            let google_play_client = runtime.block_on(get_google_play_client(
                &server_config.billing.google.service_account_key,
            ));
            let app_store_client = reqwest::Client::new();

            let index_db = Arc::new(Mutex::new(
                ServerV4::init(db_rs::Config::in_folder(&server_config.index_db.db_location))
                    .expect("Failed to load index_db"),
            ));

            let internals = InProcessInternals {
                server_state: ServerState {
                    config: server_config,
                    index_db,
                    stripe_client,
                    google_play_client,
                    app_store_client,
                },
                runtime,
            };

            Self { config, internals: Arc::new(Mutex::new(internals)) }
        }
        fn type_request<T: Request + Clone + 'static>(&self, untyped: &dyn Any) -> T {
            let request: &T = untyped.downcast_ref().unwrap();
            request.clone() // Is there a way to not clone here?
        }
    }

    // There are 2 instances of a type cast going on here, this is the cleanest solution, without
    // larger scale refactoring:
    // 1. force any request to be specifically the one that the server function we're calling expects
    // 2. force any arbitary match arm result to be the one core is expecting
    // It's the same level of unsafety-ness as above, serialization errors there are downcast errors here
    // With a larger scale refactor there could be no unsafety, if each request was aware of it's handler
    // Could be fine now that the server imports are laregly the same as the core imports. But for now,
    // this lowers the surface area of the change considerably, and all this is behind a fuzzer-only
    // compile time featuer flag
    impl Requester for InProcess {
        fn request<T: Request>(
            &self, account: &Account, request: T,
        ) -> Result<T::Response, ApiError<T::Error>> {
            let resp: Box<dyn Any> = match T::ROUTE {
                UpsertRequest::ROUTE => call!(upsert_file_metadata, self, account, request),
                ChangeDocRequest::ROUTE => call!(change_doc, self, account, request),
                GetDocRequest::ROUTE => call!(get_document, self, account, request),
                GetPublicKeyRequest::ROUTE => call!(get_public_key, self, account, request),
                GetUpdatesRequest::ROUTE => call!(get_updates, self, account, request),
                NewAccountRequest::ROUTE => call!(new_account, self, account, request),
                GetFileIdsRequest::ROUTE => call!(get_file_ids, self, account, request),
                GetUsernameRequest::ROUTE => call!(get_username, self, account, request),
                AdminValidateServerRequest::ROUTE => {
                    call!(admin_validate_server, self, account, request)
                }
                unknown => panic!("unhandled InProcess type: {}", unknown),
            };

            let resp: Result<T::Response, ServerError<T::Error>> = *resp.downcast().unwrap();

            // TODO logs can probably be re-enabled, globally, on fuzzer now, this is probably where
            // we want to capture some failures
            let resp = match resp {
                Ok(resp) => Ok(resp),
                Err(ServerError::ClientError(e)) => Err(ErrorWrapper::Endpoint(e)),
                Err(ServerError::InternalError(e)) => {
                    eprint!("internal server error {} {e}", T::ROUTE);
                    Err(ErrorWrapper::InternalError)
                }
            };

            resp.map_err(ApiError::from)
        }
    }

    #[macro_export]
    macro_rules! call {
        ($handler:path, $data:ident, $account:ident, $request:ident) => {{
            let request = $data.type_request(&$request);
            let data_internals = $data.internals.lock().unwrap();
            let server_state = &data_internals.server_state;
            let request_context = lockbook_server_lib::RequestContext {
                server_state,
                request,
                public_key: $account.public_key(),
            };
            let fut = $handler(request_context);
            Box::new(data_internals.runtime.block_on(fut))
        }};
    }

    pub type CoreIP = CoreLib<InProcess>;

    impl CoreLib<InProcess> {
        pub fn init_in_process(core_config: &Config, client: InProcess) -> Self {
            let db = CoreDb::init(db_rs::Config::in_folder(&core_config.writeable_path)).unwrap();
            let config = core_config.clone();
            let state = CoreState { config, public_key: None, db, client };
            let inner = Arc::new(Mutex::new(state));

            Self { inner }
        }

        pub fn client_config(&self) -> Config {
            self.inner.lock().unwrap().client.config.clone()
        }
    }
}
