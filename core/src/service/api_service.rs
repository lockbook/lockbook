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

#[derive(Debug, PartialEq, Eq)]
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
        let serialized_request = serde_json::to_vec(&RequestWrapper {
            signed_request,
            client_version: String::from((self.get_code_version)()),
        })
        .map_err(|err| ApiError::Serialize(err.to_string()))?;
        let serialized_response = self
            .client
            .request(T::METHOD, format!("{}{}", account.api_url, T::ROUTE).as_str())
            .body(serialized_request)
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

// #[cfg(feature = "no-network")]
mod no_network {

    use crate::call;
    use crate::service::api_service::ApiError;
    use crate::Requester;
    use lockbook_server_lib::{file_service, ServerError, ServerState};
    use lockbook_shared::account::Account;
    use lockbook_shared::api::*;
    use std::any::Any;
    use tokio::runtime::Runtime;

    pub struct InProcess {
        pub server_state: ServerState,
        pub runtime: Runtime,
    }

    impl InProcess {
        fn context<T: Request + Clone + 'static>(
            &self, account: &Account, untyped: &dyn Any,
        ) -> lockbook_server_lib::RequestContext<T> {
            let request: &T = untyped.downcast_ref().unwrap();
            let request: T = request.clone();

            lockbook_server_lib::RequestContext {
                server_state: &self.server_state,
                request,
                public_key: account.public_key(),
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
                UpsertRequest::ROUTE => call!(upsert_file_metadata, self, account, request),
                ChangeDocRequest::ROUTE => call!(change_doc, self, account, request),
                _ => panic!("unhandled InProcess type"),
            };

            let resp: Result<T::Response, ServerError<T::Error>> = *resp.downcast().unwrap();

            // TODO logs can probably be re-enabled, globally, on fuzzer now, this is probably where
            // we want to capture some failures
            let resp = match resp {
                Ok(resp) => Ok(resp),
                Err(ServerError::ClientError(e)) => Err(ErrorWrapper::Endpoint(e)),
                Err(ServerError::InternalError(_e)) => Err(ErrorWrapper::InternalError),
            };

            resp.map_err(ApiError::from)
        }
    }

    #[macro_export]
    macro_rules! call {
        ($handler:path, $data:ident, $account:ident, $request:ident) => {{
            let context = $data.context($account, &$request);
            Box::new(
                $data
                    .runtime
                    .block_on(file_service::upsert_file_metadata(context)),
            )
        }};
    }
}
