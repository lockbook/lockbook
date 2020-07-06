extern crate reqwest;

#[macro_use]
extern crate log;

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int};
use std::path::Path;

use serde_json::json;
pub use sled::Db;

use crate::client::ClientImpl;
use crate::model::crypto::DecryptedValue;
use crate::model::file_metadata::FileType::Document;
use crate::model::state::Config;
use crate::repo::account_repo::{AccountRepo, AccountRepoImpl};
use crate::repo::db_provider::{DbProvider, DiskBackedDB};
use crate::repo::document_repo::{DocumentRepo, DocumentRepoImpl};
use crate::repo::file_metadata_repo::{FileMetadataRepo, FileMetadataRepoImpl};
use crate::repo::local_changes_repo::LocalChangesRepoImpl;
use crate::service::account_service::{AccountService, AccountServiceImpl};
use crate::service::auth_service::AuthServiceImpl;
use crate::service::clock_service::ClockImpl;
use crate::service::crypto_service::{AesImpl, RsaImpl};
use crate::service::file_encryption_service::FileEncryptionServiceImpl;
use crate::service::file_service::{FileService, FileServiceImpl};
use crate::service::sync_service::{
    CalculateWorkError, FileSyncService, SyncService, WorkCalculated,
};
use crate::Error::Calculation;
use serde::export::fmt::Debug;
use serde::Serialize;
use uuid::Uuid;

pub mod client;
pub mod model;
pub mod repo;
pub mod service;

mod android;

pub static API_LOC: &str = "http://lockbook_server:8000";
pub static BUCKET_LOC: &str = "https://locked.nyc3.digitaloceanspaces.com";
static DB_NAME: &str = "lockbook.sled";

pub type DefaultCrypto = RsaImpl;
pub type DefaultSymmetric = AesImpl;
pub type DefaultDbProvider = DiskBackedDB;
pub type DefaultClient = ClientImpl;
pub type DefaultAccountRepo = AccountRepoImpl;
pub type DefaultClock = ClockImpl;
pub type DefaultAuthService = AuthServiceImpl<DefaultClock, DefaultCrypto>;
pub type DefaultAccountService = AccountServiceImpl<
    DefaultCrypto,
    DefaultAccountRepo,
    DefaultClient,
    DefaultAuthService,
    DefaultFileEncryptionService,
    DefaultFileMetadataRepo,
>;
pub type DefaultFileMetadataRepo = FileMetadataRepoImpl;
pub type DefaultLocalChangesRepo = LocalChangesRepoImpl;
pub type DefaultDocumentRepo = DocumentRepoImpl;
pub type DefaultFileEncryptionService = FileEncryptionServiceImpl<DefaultCrypto, DefaultSymmetric>;
pub type DefaultSyncService = FileSyncService<
    DefaultFileMetadataRepo,
    DefaultLocalChangesRepo,
    DefaultDocumentRepo,
    DefaultAccountRepo,
    DefaultClient,
    DefaultAuthService,
>;
pub type DefaultFileService = FileServiceImpl<
    DefaultFileMetadataRepo,
    DefaultDocumentRepo,
    DefaultLocalChangesRepo,
    DefaultAccountRepo,
    DefaultFileEncryptionService,
>;

static FAILURE_DB: &str = "FAILURE<DB_ERROR>";
static FAILURE_ACCOUNT: &str = "FAILURE<ACCOUNT_MISSING>";
static FAILURE_META_CREATE: &str = "FAILURE<META_CREATE>";
static FAILURE_FILE_GET: &str = "FAILURE<FILE_GET>";
static FAILURE_FILE_LIST: &str = "FAILURE<FILE_LIST>";
static FAILURE_ROOT_GET: &str = "FAILURE<ROOT_GET>";
static FAILURE_UUID_UNWRAP: &str = "FAILURE<UUID_UNWRAP>";

#[repr(C)]
pub struct ResultWrapper {
    is_error: bool,
    value: Value,
}

#[repr(C)]
pub union Value {
    success: *const c_char,
    error: *const c_char,
}

unsafe fn string_from_ptr(c_path: *const c_char) -> String {
    CStr::from_ptr(c_path)
        .to_str()
        .expect("Could not C String -> Rust String")
        .to_string()
}

unsafe fn connect_db(c_path: *const c_char) -> Option<Db> {
    let path = string_from_ptr(c_path);
    let config = Config {
        writeable_path: path,
    };
    match DefaultDbProvider::connect_to_db(&config) {
        Ok(db) => Some(db),
        Err(err) => {
            error!("DB connection failed! Error: {:?}", err); // TEMP HERE
            None
        }
    }
}

pub fn init_logger_safely() {
    env_logger::init();
    info!("envvar RUST_LOG is {:?}", std::env::var("RUST_LOG"));
}

#[no_mangle]
pub unsafe extern "C" fn init_logger() {
    init_logger_safely()
}

#[no_mangle]
pub unsafe extern "C" fn is_db_present(c_path: *const c_char) -> c_int {
    let path = string_from_ptr(c_path);

    let db_path = path + "/" + DB_NAME;
    debug!("Checking if {:?} exists", db_path);
    if Path::new(db_path.as_str()).exists() {
        debug!("DB Exists!");
        1
    } else {
        error!("DB Does not exist!");
        0
    }
}

#[no_mangle]
pub unsafe extern "C" fn release_pointer(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    CString::from_raw(s);
}

#[no_mangle]
pub unsafe extern "C" fn get_account(c_path: *const c_char) -> *mut c_char {
    let db = match connect_db(c_path) {
        None => return CString::new(FAILURE_DB).unwrap().into_raw(),
        Some(db) => db,
    };

    match DefaultAccountRepo::get_account(&db) {
        Ok(account) => CString::new(account.username).unwrap().into_raw(),
        Err(err) => {
            error!("Account retrieval failed with error: {:?}", err);
            CString::new(FAILURE_ACCOUNT).unwrap().into_raw()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn create_account(c_path: *const c_char, c_username: *const c_char) -> c_int {
    let db = match connect_db(c_path) {
        None => return 0,
        Some(db) => db,
    };

    let username = string_from_ptr(c_username);

    match DefaultAccountService::create_account(&db, &username) {
        Ok(_) => 1,
        Err(err) => {
            error!("Account creation failed with error: {:?}", err);
            0
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn sync_files(c_path: *const c_char) -> *mut c_char {
    let db = match connect_db(c_path) {
        None => return CString::new(FAILURE_DB).unwrap().into_raw(),
        Some(db) => db,
    };

    match DefaultSyncService::sync(&db) {
        Ok(_) => match DefaultFileMetadataRepo::get_root(&db) {
            Ok(Some(root)) => match DefaultFileMetadataRepo::get_children(&db, root.id) {
                Ok(metas) => CString::new(json!(&metas).to_string()).unwrap().into_raw(),
                Err(err3) => {
                    error!("Failed retrieving root: {:?}", err3);
                    CString::new(json!([]).to_string()).unwrap().into_raw()
                }
            },
            Ok(_) => {
                error!("No root found, you likely don't have an account!");
                CString::new(json!([]).to_string()).unwrap().into_raw()
            }
            Err(err2) => {
                error!("Failed retrieving root: {:?}", err2);
                CString::new(json!([]).to_string()).unwrap().into_raw()
            }
        },
        Err(err) => {
            error!("Sync failed with error: {:?}", err);
            CString::new(json!([]).to_string()).unwrap().into_raw()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn get_root(c_path: *const c_char) -> *mut c_char {
    let db = match connect_db(c_path) {
        None => return CString::new(FAILURE_DB).unwrap().into_raw(),
        Some(db) => db,
    };

    let out = match DefaultFileMetadataRepo::get_root(&db) {
        Ok(Some(root)) => root.id.to_string(),
        Ok(None) => FAILURE_ROOT_GET.to_string(),
        Err(err) => {
            error!("Failed to get root! Error: {:?}", err);
            FAILURE_ROOT_GET.to_string()
        }
    };

    CString::new(out.as_str()).unwrap().into_raw()
}

impl From<&str> for ResultWrapper {
    fn from(t: &str) -> Self {
        ResultWrapper {
            is_error: false,
            value: Value {
                success: CString::new(t).unwrap().into_raw(),
            },
        }
    }
}

impl<T: Serialize, E: Debug> From<Result<T, E>> for ResultWrapper {
    fn from(result: Result<T, E>) -> Self {
        ResultWrapper {
            is_error: result.is_err(),
            value: {
                match result {
                    Ok(value) => Value {
                        success: CString::new(json!(value).to_string()).unwrap().into_raw(),
                    },
                    Err(err) => Value {
                        error: CString::new(format!("{:?}", err)).unwrap().into_raw(),
                    },
                }
            },
        }
    }
}

#[derive(Debug)]
enum Error {
    General(repo::db_provider::Error),
    Calculation(CalculateWorkError),
}

#[no_mangle]
pub unsafe extern "C" fn calculate_work(c_path: *const c_char) -> ResultWrapper {
    unsafe fn inner(path: String) -> Result<WorkCalculated, Error> {
        let db = DefaultDbProvider::connect_to_db(&Config {
            writeable_path: path,
        })
        .map_err(Error::General)?;

        let work = DefaultSyncService::calculate_work(&db).map_err(Calculation)?;

        Ok(work)
    }

    ResultWrapper::from(inner(string_from_ptr(c_path)))
}

#[no_mangle]
pub unsafe extern "C" fn list_files(
    c_path: *const c_char,
    c_parent_id: *const c_char,
) -> *mut c_char {
    let db = match connect_db(c_path) {
        None => return CString::new(FAILURE_DB).unwrap().into_raw(),
        Some(db) => db,
    };
    let parent_id = string_from_ptr(c_parent_id);

    match Uuid::parse_str(parent_id.as_str()) {
        Ok(parent_uuid) => match DefaultFileMetadataRepo::get_children(&db, parent_uuid) {
            Ok(metas) => CString::new(json!(&metas).to_string()).unwrap().into_raw(),
            Err(err) => {
                error!(
                    "Failure while get children of {}! Error: {:?}",
                    parent_uuid, err
                );
                CString::new(FAILURE_FILE_LIST).unwrap().into_raw()
            }
        },
        Err(uuid_parse_error) => {
            error!(
                "Failure parsing {} into UUID! Error: {:?}",
                parent_id, uuid_parse_error
            );
            CString::new(FAILURE_UUID_UNWRAP).unwrap().into_raw()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn create_file(
    c_path: *const c_char,
    c_file_name: *const c_char,
    c_file_parent_id: *const c_char, // TODO @raayan add type?
) -> *mut c_char {
    let db = match connect_db(c_path) {
        None => return CString::new(FAILURE_DB).unwrap().into_raw(),
        Some(db) => db,
    };
    let file_name = string_from_ptr(c_file_name);
    let file_parent_id = string_from_ptr(c_file_parent_id);

    let file_parent_uuid: Uuid = match Uuid::parse_str(&file_parent_id) {
        Ok(uuid) => uuid,
        Err(err) => {
            error!("Failed to create file metadata! Error: {:?}", err);
            return CString::new(FAILURE_UUID_UNWRAP).unwrap().into_raw();
        }
    };

    match DefaultFileService::create(
        &db,
        &file_name,
        file_parent_uuid,
        Document, // TODO @raayan
    ) {
        Ok(meta) => CString::new(json!(&meta).to_string()).unwrap().into_raw(),
        Err(err) => {
            error!("Failed to create file metadata! Error: {:?}", err);
            CString::new(FAILURE_META_CREATE).unwrap().into_raw()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn get_file(c_path: *const c_char, c_file_id: *const c_char) -> *mut c_char {
    let db = match connect_db(c_path) {
        None => return CString::new(FAILURE_DB).unwrap().into_raw(),
        Some(db) => db,
    };
    let file_id = string_from_ptr(c_file_id);

    match DefaultFileService::read_document(&db, Uuid::parse_str(file_id.as_str()).unwrap()) {
        Ok(file) => CString::new(json!(&file).to_string()).unwrap().into_raw(),
        Err(err) => {
            error!("Failed to get file! Error: {:?}", err);
            CString::new(FAILURE_FILE_GET).unwrap().into_raw()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn update_file(
    c_path: *const c_char,
    c_file_id: *const c_char,
    c_file_content: *const c_char,
) -> c_int {
    let db = match connect_db(c_path) {
        None => return 0,
        Some(db) => db,
    };
    let file_id = string_from_ptr(c_file_id);
    let file_content = DecryptedValue {
        secret: string_from_ptr(c_file_content),
    };

    match DefaultFileService::write_document(
        &db,
        Uuid::parse_str(file_id.as_str()).unwrap(),
        &file_content,
    ) {
        Ok(_) => 1,
        Err(err) => {
            error!("Failed to update file! Error: {:?}", err);
            0
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn mark_file_for_deletion(
    c_path: *const c_char,
    c_file_id: *const c_char,
) -> c_int {
    let _ = match connect_db(c_path) {
        None => return 0,
        Some(db) => db,
    };
    let file_id = string_from_ptr(c_file_id);

    error!(
        "You tried to delete {} but we don't support that right now!",
        file_id
    );

    // TODO: @raayan implement this when there's a good way to delete files
    return 0;
}

/// DEBUG FUNCTIONS
#[no_mangle]
pub unsafe extern "C" fn purge_files(c_path: *const c_char) -> c_int {
    let db = match connect_db(c_path) {
        None => return 0,
        Some(db) => db,
    };
    match DefaultFileMetadataRepo::get_all(&db) {
        Ok(metas) => metas.into_iter().for_each(|meta| {
            DefaultFileMetadataRepo::actually_delete(&db, meta.id).unwrap();
            DefaultDocumentRepo::delete(&db, meta.id).unwrap();
        }),
        Err(err) => error!("Failed to delete file! Error: {:?}", err),
    }
    1
}

#[no_mangle]
pub unsafe extern "C" fn import_account(c_path: *const c_char, c_account: *const c_char) -> c_int {
    let db = match connect_db(c_path) {
        None => return 0,
        Some(db) => db,
    };
    let account_string = string_from_ptr(c_account);
    match DefaultAccountService::import_account(&db, &account_string) {
        Ok(acc) => {
            debug!("Loaded account: {:?}", acc);
            1
        }
        Err(err) => {
            error!("Failed to delete file! Error: {:?}", err);
            0
        }
    }
}
