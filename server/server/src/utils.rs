use crate::ServerError;
use lockbook_shared::api::{GetBuildInfoError, GetBuildInfoResponse};
use shadow_rs::shadow;

shadow!(build_info);

pub fn username_is_valid(username: &str) -> bool {
    !username.is_empty()
        && username
            .to_lowercase()
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
}

pub fn get_build_info() -> Result<GetBuildInfoResponse, ServerError<GetBuildInfoError>> {
    Ok(GetBuildInfoResponse {
        build_version: env!("CARGO_PKG_VERSION").to_string(),
        git_commit_hash: build_info::COMMIT_HASH.to_string(),
    })
}
