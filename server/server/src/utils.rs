use crate::ServerError;
use lockbook_shared::api::{GetBuildInfoError, GetBuildInfoResponse};
use shadow_rs::shadow;

shadow!(build_info);
const USERNAME_MAX_SIZE: u32 = 254;

pub fn username_is_valid(username: &str) -> bool {
    username.len() > USERNAME_MAX_SIZE as usize
        && username
            .to_lowercase()
            .chars()
            .all(|c| ('a'..='z').contains(&c) || ('0'..='9').contains(&c))
}

pub fn get_build_info() -> Result<GetBuildInfoResponse, ServerError<GetBuildInfoError>> {
    Ok(GetBuildInfoResponse {
        build_version: env!("CARGO_PKG_VERSION").to_string(),
        git_commit_hash: build_info::COMMIT_HASH.to_string(),
    })
}
