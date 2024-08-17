use reqwest::blocking::Client as RequestClient;

use crate::get_code_version;
use crate::shared::account::Account;
use crate::shared::api::*;
use crate::shared::clock::{get_time, Timestamp};
use crate::shared::pubkey;

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
    Sign(crate::shared::SharedError),
    Serialize(String),
    SendFailed(String),
    ReceiveFailed(String),
    Deserialize(String),
}

pub trait Requester: Clone + Send + 'static {
    fn request<T: Request>(
        &self, account: &Account, request: T,
    ) -> Result<T::Response, ApiError<T::Error>>;
}

#[derive(Debug, Clone)]
#[repr(C)]
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
        let a = self
            .client
            .request(T::METHOD, format!("{}{}", account.api_url, T::ROUTE).as_str())
            .body(serialized_request)
            .header("Accept-Version", client_version)
            .send()
            .map_err(|err| {
                warn!("Send failed: {:#?}", err);
                ApiError::SendFailed(err.to_string())
            })?;
        let serialized_response = a
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
    use crate::shared::account::Account;
    use crate::shared::api::*;
    use crate::shared::core_config::Config;
    use crate::shared::crypto::EncryptedDocument;
    use crate::shared::document_repo::DocumentService;
    use crate::shared::file_metadata::DocumentHmac;
    use crate::{call, CoreLib, CoreState};
    use crate::{CoreDb, Requester};
    use db_rs::Db;
    use lockbook_server_lib::billing::Nop;
    use lockbook_server_lib::config::*;
    use lockbook_server_lib::document_service::InMemDocuments;
    use lockbook_server_lib::schema::ServerV4;
    use lockbook_server_lib::{ServerError, ServerState};
    use std::any::Any;
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::time::Duration;
    use tokio::runtime;
    use tokio::runtime::Runtime;
    use uuid::Uuid;

    #[derive(Clone)]
    pub struct InProcess {
        pub config: Config,
        pub internals: Arc<Mutex<InProcessInternals>>,
    }

    pub struct InProcessInternals {
        pub server_state: ServerState<Nop, Nop, Nop, InMemDocuments>,
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

            let stripe_client = Nop {};
            let google_play_client = Nop {};
            let app_store_client = Nop {};

            let index_db = Arc::new(Mutex::new(
                ServerV4::init(db_rs::Config::no_io()).expect("Failed to load index_db"),
            ));
            let document_service = InMemDocuments::default();

            let internals = InProcessInternals {
                server_state: ServerState {
                    config: server_config,
                    index_db,
                    stripe_client,
                    google_play_client,
                    app_store_client,
                    document_service,
                },
                runtime,
            };

            Self { config, internals: Arc::new(Mutex::new(internals)) }
        }

        fn type_request<T: Request + Clone + 'static>(&self, untyped: &dyn Any) -> T {
            let request: &T = untyped.downcast_ref().unwrap();
            request.clone() // Is there a way to not clone here?
        }

        pub fn deep_copy(&self) -> Self {
            let internals = self.internals.lock().unwrap();
            let mut server_state = internals.server_state.clone();
            let docs_db_clone = server_state.document_service.docs.lock().unwrap().clone();
            let server_db_clone = server_state.index_db.lock().unwrap().clone();

            server_state.index_db = Arc::new(Mutex::new(server_db_clone));
            server_state.document_service =
                InMemDocuments { docs: Arc::new(Mutex::new(docs_db_clone)) };

            Self {
                config: self.config.clone(),
                internals: Arc::new(Mutex::new(InProcessInternals {
                    server_state,
                    runtime: runtime::Builder::new_current_thread().build().unwrap(),
                })),
            }
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
                UpsertRequest::ROUTE => {
                    call!(ServerState::upsert_file_metadata, self, account, request)
                }
                ChangeDocRequest::ROUTE => call!(ServerState::change_doc, self, account, request),
                GetDocRequest::ROUTE => call!(ServerState::get_document, self, account, request),
                GetPublicKeyRequest::ROUTE => {
                    call!(ServerState::get_public_key, self, account, request)
                }
                GetUpdatesRequest::ROUTE => call!(ServerState::get_updates, self, account, request),
                NewAccountRequest::ROUTE => call!(ServerState::new_account, self, account, request),
                GetFileIdsRequest::ROUTE => {
                    call!(ServerState::get_file_ids, self, account, request)
                }
                GetUsernameRequest::ROUTE => {
                    call!(ServerState::get_username, self, account, request)
                }
                AdminValidateServerRequest::ROUTE => {
                    call!(ServerState::admin_validate_server, self, account, request)
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
            let request_context =
                lockbook_server_lib::RequestContext { request, public_key: $account.public_key() };
            let fut = $handler(server_state, request_context);
            Box::new(data_internals.runtime.block_on(fut))
        }};
    }

    pub type CoreIP = CoreLib<InProcess, CoreInMemDocuments>;

    impl CoreLib<InProcess, CoreInMemDocuments> {
        pub fn init_in_process(core_config: &Config, client: InProcess) -> Self {
            let db = CoreDb::init(db_rs::Config::no_io()).unwrap();
            let config = core_config.clone();
            let docs = CoreInMemDocuments::default();
            let syncing = false;
            let state = CoreState { config, public_key: None, db, client, docs, syncing };
            let inner = Arc::new(Mutex::new(state));

            Self { inner }
        }

        pub fn client_config(&self) -> Config {
            self.inner.lock().unwrap().client.config.clone()
        }

        pub fn deep_copy(&self) -> (Self, InProcess) {
            let inner = self.inner.lock().unwrap();
            let config = inner.config.clone();
            let db = inner.db.clone();
            let client = inner.client.deep_copy();
            let docs = inner.docs.docs.lock().unwrap().clone();
            let docs = CoreInMemDocuments { docs: Arc::new(Mutex::new(docs)) };
            let syncing = inner.syncing;
            let state = CoreState {
                config,
                public_key: inner.public_key,
                db,
                docs,
                client: client.clone(),
                syncing,
            };
            (Self { inner: Arc::new(Mutex::new(state)) }, client)
        }

        pub fn set_client(&self, c: InProcess) {
            self.inner.lock().unwrap().client = c
        }
    }

    #[derive(Default, Clone)]
    pub struct CoreInMemDocuments {
        docs: Arc<Mutex<HashMap<String, EncryptedDocument>>>,
    }

    impl DocumentService for CoreInMemDocuments {
        fn insert(
            &self, id: &uuid::Uuid, hmac: Option<&crate::shared::file_metadata::DocumentHmac>,
            document: &EncryptedDocument,
        ) -> crate::shared::SharedResult<()> {
            if let Some(hmac) = hmac {
                let hmac = base64::encode_config(hmac, base64::URL_SAFE);
                let key = format!("{id}-{hmac}");
                self.docs.lock().unwrap().insert(key, document.clone());
            }
            Ok(())
        }

        fn maybe_get(
            &self, id: &uuid::Uuid, hmac: Option<&crate::shared::file_metadata::DocumentHmac>,
        ) -> crate::shared::SharedResult<Option<EncryptedDocument>> {
            if let Some(hmac) = hmac {
                let hmac = base64::encode_config(hmac, base64::URL_SAFE);
                let key = format!("{id}-{hmac}");
                Ok(self.docs.lock().unwrap().get(&key).cloned())
            } else {
                Ok(None)
            }
        }

        fn delete(
            &self, id: &uuid::Uuid, hmac: Option<&crate::shared::file_metadata::DocumentHmac>,
        ) -> crate::shared::SharedResult<()> {
            if let Some(hmac) = hmac {
                let hmac = base64::encode_config(hmac, base64::URL_SAFE);
                let key = format!("{id}-{hmac}");
                self.docs.lock().unwrap().remove(&key);
            }
            Ok(())
        }

        fn retain(
            &self, file_hmacs: HashSet<(&Uuid, &DocumentHmac)>,
        ) -> crate::shared::SharedResult<()> {
            let mut keep_keys = HashSet::new();
            for (id, hmac) in file_hmacs {
                let hmac = base64::encode_config(hmac, base64::URL_SAFE);
                let key = format!("{id}-{hmac}");
                keep_keys.insert(key);
            }

            let mut delete_keys = vec![];
            let docs = self.docs.lock().unwrap();
            for key in docs.keys() {
                if !keep_keys.contains(key) {
                    delete_keys.push(key.clone());
                }
            }
            drop(docs);

            let mut docs = self.docs.lock().unwrap();
            for key in delete_keys {
                docs.remove(&key);
            }

            Ok(())
        }
    }
}
