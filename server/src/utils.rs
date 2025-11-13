use crate::ServerError;
use lb_rs::model::account::MAX_USERNAME_LENGTH;
use lb_rs::model::api::{GetBuildInfoError, GetBuildInfoResponse};
use shadow_rs::shadow;

shadow!(build_info);

pub fn username_is_valid(username: &str) -> bool {
    !username.is_empty()
        && username.len() <= MAX_USERNAME_LENGTH
        && username.to_lowercase().chars().all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_' || c == '.'
        })
}

pub fn get_build_info() -> Result<GetBuildInfoResponse, ServerError<GetBuildInfoError>> {
    Ok(GetBuildInfoResponse {
        build_version: env!("CARGO_PKG_VERSION").to_string(),
        git_commit_hash: build_info::COMMIT_HASH.to_string(),
    })
}
