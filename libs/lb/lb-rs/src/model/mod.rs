pub mod access_info;
pub mod account;
pub mod api;
pub mod clock;
pub mod compression_service;
pub mod core_config;
pub mod core_ops;
pub mod core_tree;
pub mod crypto;
pub mod errors;
pub mod feature_flag;
pub mod file;
pub mod file_like;
pub mod file_metadata;
pub mod filename;
pub mod lazy;
pub mod path_ops;
pub mod pubkey;
pub mod secret_filename;
pub mod server_file;
pub mod server_ops;
pub mod server_tree;
pub mod signed_file;
pub mod staged;
pub mod svg;
pub mod symkey;
pub mod text;
pub mod tree_like;
pub mod usage;
pub mod validate;
pub mod work_unit;

pub use lazy::ValidationFailure;

use std::backtrace::Backtrace;
use std::io;

use db_rs::DbError;

pub type SharedResult<T> = Result<T, SharedError>;

#[derive(Debug)]
pub struct SharedError {
    pub kind: SharedErrorKind,
    pub backtrace: Option<Backtrace>,
}

impl From<SharedErrorKind> for SharedError {
    fn from(kind: SharedErrorKind) -> Self {
        Self { kind, backtrace: Some(Backtrace::force_capture()) }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum SharedErrorKind {}
