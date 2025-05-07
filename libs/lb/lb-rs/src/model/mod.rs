//! Members of this module specialize in "pure" representation of ideas
//! these could be just where our data model lives or could be expressions
//! of complicated algorithmic or abstract ideas. If you're writing code
//! in this module it's expected that you optimize for portability by not
//! using IO, not using async, and generally staying away from locks. It's
//! expected that code in this module is reasonably easy to test as well.

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
pub mod meta;
pub mod meta_conversions;
pub mod path_ops;
pub mod pubkey;
pub mod secret_filename;
pub mod server_file;
pub mod server_meta;
pub mod server_ops;
pub mod server_tree;
pub mod signed_file;
pub mod signed_meta;
pub mod staged;
pub mod svg;
pub mod symkey;
pub mod text;
pub mod tree_like;
pub mod usage;
pub mod validate;
pub mod work_unit;

pub use lazy::ValidationFailure;
