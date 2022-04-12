use serde::Serialize;

use crate::model::state::Config;
use crate::repo::{account_repo, db_version_repo};
use crate::CoreError;

pub fn get_code_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
