use lockbook_models::api::{GetBuildInfoError, GetBuildInfoResponse};
use crate::ServerError;

use shadow_rs::shadow;
shadow!(build_info);

pub fn username_is_valid(username: &str) -> bool {
    !username.is_empty()
        && username
            .to_lowercase()
            .chars()
            .all(|c| ('a'..='z').contains(&c) || ('0'..='9').contains(&c))
}

pub async fn get_build_info() -> Result<GetBuildInfoResponse, ServerError<GetBuildInfoError>> {
    Ok(GetBuildInfoResponse {
        build_version: env!("CARGO_PKG_VERSION"),
        git_commit_hash: build_info::COMMIT_HASH,
    })
}