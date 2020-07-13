extern crate reqwest;

#[macro_use]
extern crate log;

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::path::Path;

use serde_json::json;
pub use sled::Db;

use crate::client::ClientImpl;
use crate::model::account::{Account, Username};
use crate::model::crypto::DecryptedValue;
use crate::model::file_metadata::FileMetadata;
use crate::model::file_metadata::FileType::{Document, Folder};
use crate::model::state::Config;
use crate::model::work_unit::WorkUnit;
use crate::repo::account_repo::{AccountRepo, AccountRepoImpl};
use crate::repo::db_provider::{DbProvider, DiskBackedDB};
use crate::repo::document_repo::DocumentRepoImpl;
use crate::repo::file_metadata_repo::{FileMetadataRepo, FileMetadataRepoImpl};
use crate::repo::local_changes_repo::LocalChangesRepoImpl;
use crate::service::account_service::{AccountService, AccountServiceImpl};
use crate::service::auth_service::AuthServiceImpl;
use crate::service::clock_service::ClockImpl;
use crate::service::crypto_service::{AesImpl, RsaImpl};
use crate::service::file_encryption_service::FileEncryptionServiceImpl;
use crate::service::file_service::{FileService, FileServiceImpl};
use crate::service::sync_service::{FileSyncService, SyncService};
use serde::export::fmt::Debug;
use serde::Serialize;
use uuid::Uuid;

pub mod client;
pub mod model;
pub mod repo;
pub mod service;

mod android;

pub static API_LOC: &str = "http://qa.lockbook.app:8000";
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

#[repr(C)]
pub struct ResultWrapper {
    is_error: bool,
    value: Value,
    error: LockbookError,
}

#[repr(C)]
pub union Value {
    success: *const c_char,
    error: *const c_char,
}

// TODO: make these better (they're the errors clients will actually handle)
#[repr(C)]
pub enum LockbookError {
    Network,
    Database,
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
            error: LockbookError::Database,
        }
    }
}

impl From<uuid::Error> for Error {
    fn from(err: uuid::Error) -> Self {
        Self::Uuid(err)
    }
}

#[derive(Debug)]
enum Error {
    // Uncategorized, // TODO: ideally nothing is in here, but we know that can be hard
    Db(repo::db_provider::Error),
    Metas(repo::file_metadata_repo::DbError),
    Uuid(uuid::Error),
    Json(serde_json::Error),
    Sync(service::sync_service::SyncError),
    Calculation(service::sync_service::CalculateWorkError),
    Execution(service::sync_service::WorkExecutionError),
    AccountCreate(service::account_service::AccountCreationError),
    AccountRetrieve(repo::account_repo::Error),
    AccountImport(service::account_service::AccountImportError),
    FileCreate(service::file_service::NewFileError),
    FileRetrieve(service::file_service::ReadDocumentError),
    FileUpdate(service::file_service::DocumentUpdateError),
    Unimplemented,
    NoRoot,
}

unsafe fn from_ptr(c_path: *const c_char) -> String {
    CStr::from_ptr(c_path)
        .to_str()
        .expect("Could not C String -> Rust String")
        .to_string()
}

pub fn init_logger_safely() {
    env_logger::init();
    info!("envvar RUST_LOG is {:?}", std::env::var("RUST_LOG"));
}

#[no_mangle]
pub unsafe extern "C" fn init_logger() {
    init_logger_safely()
}

unsafe fn connect(path: String) -> Result<Db, Error> {
    let config = Config {
        writeable_path: path,
    };
    DefaultDbProvider::connect_to_db(&config).map_err(Error::Db)
}

#[no_mangle]
pub unsafe extern "C" fn is_db_present(c_path: *const c_char) -> bool {
    let path = from_ptr(c_path);
    let db_path = path + "/" + DB_NAME;
    debug!("Checking if {:?} exists", db_path);
    Path::new(db_path.as_str()).exists()
}

#[no_mangle]
pub unsafe extern "C" fn release_pointer(s: *mut c_char) {
    if s.is_null() {
        return;
    }
    CString::from_raw(s);
}

/// Account

#[no_mangle]
pub unsafe extern "C" fn get_account(c_path: *const c_char) -> ResultWrapper {
    unsafe fn inner(path: String) -> Result<Username, Error> {
        let db = connect(path)?;
        DefaultAccountRepo::get_account(&db)
            .map(|a| a.username)
            .map_err(Error::AccountRetrieve)
    }
    ResultWrapper::from(inner(from_ptr(c_path)))
}

#[no_mangle]
pub unsafe extern "C" fn create_account(
    c_path: *const c_char,
    c_username: *const c_char,
) -> ResultWrapper {
    unsafe fn inner(path: String, username: String) -> Result<Account, Error> {
        let db = connect(path)?;
        DefaultAccountService::create_account(&db, &username).map_err(Error::AccountCreate)
    }
    ResultWrapper::from(inner(from_ptr(c_path), from_ptr(c_username)))
}

#[no_mangle]
pub unsafe extern "C" fn import_account(
    c_path: *const c_char,
    c_account: *const c_char,
) -> ResultWrapper {
    unsafe fn inner(path: String, account_string: String) -> Result<Account, Error> {
        let db = connect(path)?;
        DefaultAccountService::import_account(&db, &account_string).map_err(Error::AccountImport)
    }
    ResultWrapper::from(inner(from_ptr(c_path), from_ptr(c_account)))
}

/// Work

#[no_mangle]
pub unsafe extern "C" fn sync_files(c_path: *const c_char) -> ResultWrapper {
    unsafe fn inner(path: String) -> Result<bool, Error> {
        let db = connect(path)?;
        DefaultSyncService::sync(&db)
            .map_err(Error::Sync)
            .map(|_| true)
    }
    ResultWrapper::from(inner(from_ptr(c_path)))
}

#[no_mangle]
pub unsafe extern "C" fn calculate_work(c_path: *const c_char) -> ResultWrapper {
    unsafe fn inner(path: String) -> Result<Vec<WorkUnit>, Error> {
        let db = connect(path)?;
        let work = DefaultSyncService::calculate_work(&db).map_err(Error::Calculation)?;
        Ok(work.work_units)
    }
    ResultWrapper::from(inner(from_ptr(c_path)))
}

#[no_mangle]
pub unsafe extern "C" fn execute_work(
    c_path: *const c_char,
    c_work_unit: *const c_char,
) -> ResultWrapper {
    unsafe fn inner(path: String, work_str: String) -> Result<bool, Error> {
        let db = connect(path)?;
        let work: WorkUnit = serde_json::from_str(&work_str).map_err(Error::Json)?;
        let account = DefaultAccountRepo::get_account(&db).map_err(Error::AccountRetrieve)?;
        DefaultSyncService::execute_work(&db, &account, work)
            .map_err(Error::Execution)
            .map(|_| true)
    }
    ResultWrapper::from(inner(from_ptr(c_path), from_ptr(c_work_unit)))
}

/// Directory

#[no_mangle]
pub unsafe extern "C" fn get_root(c_path: *const c_char) -> ResultWrapper {
    unsafe fn inner(path: String) -> Result<FileMetadata, Error> {
        let db = connect(path)?;
        DefaultFileMetadataRepo::get_root(&db)
            .map_err(Error::Metas)?
            .ok_or(Error::NoRoot)
    }
    ResultWrapper::from(inner(from_ptr(c_path)))
}

#[no_mangle]
pub unsafe extern "C" fn list_files(
    c_path: *const c_char,
    c_parent_id: *const c_char,
) -> ResultWrapper {
    unsafe fn inner(path: String, parent_id: String) -> Result<Vec<FileMetadata>, Error> {
        let db = connect(path)?;
        let parent_uuid = Uuid::parse_str(parent_id.as_str()).map_err(Error::Uuid)?;
        DefaultFileMetadataRepo::get_children(&db, parent_uuid).map_err(Error::Metas)
    }
    ResultWrapper::from(inner(from_ptr(c_path), from_ptr(c_parent_id)))
}

/// Document

#[no_mangle]
pub unsafe extern "C" fn get_file(
    c_path: *const c_char,
    c_file_id: *const c_char,
) -> ResultWrapper {
    unsafe fn inner(path: String, file_id: String) -> Result<DecryptedValue, Error> {
        let db = connect(path)?;
        let file_uuid = Uuid::parse_str(file_id.as_str())?;
        DefaultFileService::read_document(&db, file_uuid).map_err(Error::FileRetrieve)
    }
    ResultWrapper::from(inner(from_ptr(c_path), from_ptr(c_file_id)))
}

#[no_mangle]
pub unsafe extern "C" fn create_file(
    c_path: *const c_char,
    c_file_name: *const c_char,
    c_file_parent_id: *const c_char,
    c_is_folder: bool,
) -> ResultWrapper {
    unsafe fn inner(
        path: String,
        file_name: String,
        file_parent: String,
        is_folder: bool,
    ) -> Result<FileMetadata, Error> {
        let db = connect(path)?;
        let file_parent_uuid = Uuid::parse_str(&file_parent)?;
        // TODO @raayan make this function work for docs & folders
        let file_type = if is_folder { Folder } else { Document };
        DefaultFileService::create(&db, &file_name, file_parent_uuid, file_type)
            .map_err(Error::FileCreate)
    }
    ResultWrapper::from(inner(
        from_ptr(c_path),
        from_ptr(c_file_name),
        from_ptr(c_file_parent_id),
        c_is_folder,
    ))
}

#[no_mangle]
pub unsafe extern "C" fn update_file(
    c_path: *const c_char,
    c_file_id: *const c_char,
    c_file_content: *const c_char,
) -> ResultWrapper {
    unsafe fn inner(path: String, file_id: String, file_content: String) -> Result<bool, Error> {
        let db = connect(path)?;
        let file_uuid = Uuid::parse_str(file_id.as_str())?;
        let value = &DecryptedValue {
            secret: file_content,
        };
        DefaultFileService::write_document(&db, file_uuid, value)
            .map_err(Error::FileUpdate)
            .map(|_| true)
    }
    ResultWrapper::from(inner(
        from_ptr(c_path),
        from_ptr(c_file_id),
        from_ptr(c_file_content),
    ))
}

#[no_mangle]
pub unsafe extern "C" fn mark_file_for_deletion(
    c_path: *const c_char,
    c_file_id: *const c_char,
) -> ResultWrapper {
    unsafe fn inner(path: String, file_id: String) -> Result<bool, Error> {
        let _ = connect(path)?;
        let _ = Uuid::parse_str(file_id.as_str())?;
        Err(Error::Unimplemented)
    }
    // TODO: @raayan implement this when there's a good way to delete files
    ResultWrapper::from(inner(from_ptr(c_path), from_ptr(c_file_id)))
}
