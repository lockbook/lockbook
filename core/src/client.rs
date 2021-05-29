use crate::service::db_state_service::get_code_version;
use lockbook_crypto::clock_service::{get_time, Timestamp};
use lockbook_crypto::pubkey;
use lockbook_crypto::pubkey::ECSignError;
use lockbook_models::account::Account;
use lockbook_models::api::*;
use reqwest::blocking::Client as ReqwestClient;
use reqwest::Error as ReqwestError;
use serde::de::DeserializeOwned;
use serde::Serialize;

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

#[derive(Debug)]
pub enum ApiError<E> {
    Endpoint(E),
    ClientUpdateRequired,
    InvalidAuth,
    ExpiredAuth,
    InternalError,
    BadRequest,
    Sign(ECSignError),
    Serialize(serde_json::error::Error),
    SendFailed(ReqwestError),
    ReceiveFailed(ReqwestError),
    Deserialize(serde_json::error::Error),
}

pub fn request<
    T: Request<Response = impl DeserializeOwned, Error = impl DeserializeOwned> + Serialize,
>(
    account: &Account,
    request: T,
) -> Result<T::Response, ApiError<T::Error>> {
    request_helper(account, request, get_code_version, get_time)
}

fn request_helper<
    T: Request<Response = impl DeserializeOwned, Error = impl DeserializeOwned> + Serialize,
>(
    account: &Account,
    request: T,
    get_code_version: fn() -> &'static str,
    get_time: fn() -> Timestamp,
) -> Result<T::Response, ApiError<T::Error>> {
    let client = ReqwestClient::new();
    let signed_request =
        pubkey::sign(&account.private_key, request, get_time).map_err(ApiError::Sign)?;
    let serialized_request = serde_json::to_vec(&RequestWrapper {
        signed_request,
        client_version: String::from(get_code_version()),
    })
    .map_err(ApiError::Serialize)?;
    let serialized_response = client
        .request(
            T::METHOD,
            format!("{}{}", account.api_url, T::ROUTE).as_str(),
        )
        .body(serialized_request)
        .send()
        .map_err(ApiError::SendFailed)?
        .bytes()
        .map_err(ApiError::ReceiveFailed)?;
    let response: Result<T::Response, ErrorWrapper<T::Error>> =
        serde_json::from_slice(&serialized_response).map_err(ApiError::Deserialize)?;
    response.map_err(ApiError::from)
}

#[cfg(test)]
mod request_common_tests {

    // TODO make a test only crate
    pub fn random_username() -> String {
        Uuid::new_v4()
            .to_string()
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect()
    }

    pub fn generate_account() -> Account {
        Account {
            username: random_username(),
            api_url: env::var("API_URL").expect("API_URL must be defined!"),
            private_key: generate_key(),
        }
    }

    pub fn aes_encrypt<T: Serialize + DeserializeOwned>(
        key: &AESKey,
        to_encrypt: &T,
    ) -> AESEncrypted<T> {
        AESImpl::encrypt(key, to_encrypt).unwrap()
    }

    // TODO make a test only crate
    pub fn generate_root_metadata(account: &Account) -> (FileMetadata, AESKey) {
        let id = Uuid::new_v4();
        let folder_key = AESImpl::generate_key();

        let public_key = account.public_key();
        let user_access_info = UserAccessInfo {
            username: account.username.clone(),
            encrypted_by: public_key.clone(),
            access_key: aes_encrypt(
                &pubkey::get_aes_key(&account.private_key, &account.public_key()).unwrap(),
                &folder_key,
            ),
        };
        let mut user_access_keys = HashMap::new();
        user_access_keys.insert(account.username.clone(), user_access_info);

        (
            FileMetadata {
                file_type: FileType::Folder,
                id,
                name: account.username.clone(),
                owner: account.username.clone(),
                parent: id,
                content_version: 0,
                metadata_version: 0,
                deleted: false,
                user_access_keys,
                folder_access_keys: FolderAccessInfo {
                    folder_id: id,
                    access_key: aes_encrypt(&folder_key, &folder_key),
                },
            },
            folder_key,
        )
    }

    #[macro_export]
    macro_rules! assert_matches (
    ($actual:expr, $expected:pat) => {
        // Only compute actual once
        let actual_value = $actual;
        match actual_value {
            $expected => {},
            _ => panic!("assertion failed: {:?} did not match expectation", actual_value)
            }
        }
    );

    use crate::{create_account, get_account};
    use libsecp256k1::PublicKey;
    use lockbook_crypto::clock_service::{get_time, Timestamp};

    use lockbook_models::api::{
        GetPublicKeyError, GetPublicKeyRequest, GetPublicKeyResponse, NewAccountError,
        NewAccountRequest,
    };

    use crate::client::{request_helper, ApiError};
    use crate::model::state::temp_config;
    use crate::service::db_state_service::get_code_version;
    use lockbook_crypto::pubkey;
    use lockbook_crypto::pubkey::generate_key;
    use lockbook_crypto::symkey::{AESImpl, SymmetricCryptoService};
    use lockbook_models::account::Account;
    use lockbook_models::crypto::{AESEncrypted, AESKey, FolderAccessInfo, UserAccessInfo};
    use lockbook_models::file_metadata::{FileMetadata, FileType};
    use serde::de::DeserializeOwned;
    use serde::Serialize;
    use std::collections::HashMap;
    use std::env;
    use uuid::Uuid;

    static CODE_VERSION: fn() -> &'static str = || "0.0.0";

    #[test]
    fn forced_upgrade() {
        let cfg = temp_config();
        let generated_account = generate_account();
        create_account(
            &cfg,
            &generated_account.username,
            &generated_account.api_url,
        )
        .unwrap();
        let account = get_account(&cfg).unwrap();

        let result: Result<PublicKey, ApiError<GetPublicKeyError>> = request_helper(
            &account,
            GetPublicKeyRequest {
                username: account.username.clone(),
            },
            CODE_VERSION,
            get_time,
        )
        .map(|r: GetPublicKeyResponse| r.key);

        assert_matches!(
            result,
            Err(ApiError::<GetPublicKeyError>::ClientUpdateRequired)
        );
    }

    static EARLY_CLOCK: fn() -> Timestamp = || Timestamp(get_time().0 - 3600000);

    #[test]
    fn expired_request() {
        let account = generate_account();
        let (root, _) = generate_root_metadata(&account);

        let result = request_helper(
            &account,
            NewAccountRequest::new(&account, &root),
            get_code_version,
            EARLY_CLOCK,
        );
        assert_matches!(result, Err(ApiError::<NewAccountError>::ExpiredAuth));
    }

    // TODO add a test for bad signatures
}
