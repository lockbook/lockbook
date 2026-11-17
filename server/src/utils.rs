use crate::ServerError;
use lb_rs::model::account::{MAX_USERNAME_LENGTH, SUB_ACCOUNT_SEPARATOR};
use lb_rs::model::api::{GetBuildInfoError, GetBuildInfoResponse};

pub fn username_is_valid(username: &str) -> bool {
    !username.is_empty()
        && username.len() <= MAX_USERNAME_LENGTH
        && username.to_lowercase().chars().all(|c| {
            c.is_ascii_lowercase()
                || c.is_ascii_digit()
                || c == '-'
                || c == '_'
                || c == '.'
                || c == '@'
        })
}

pub fn sub_account_username_is_valid(parent: &str, username: &str) -> bool {
    username.len() <= MAX_USERNAME_LENGTH
        && username
            .strip_prefix(parent)
            .and_then(|rest| rest.strip_prefix(SUB_ACCOUNT_SEPARATOR))
            .is_some_and(username_is_valid)
}

pub fn get_build_info() -> Result<GetBuildInfoResponse, ServerError<GetBuildInfoError>> {
    Ok(GetBuildInfoResponse {
        build_version: env!("CARGO_PKG_VERSION").to_string(),
        git_commit_hash: lb_rs::get_code_commit().unwrap_or_default().to_string(),
    })
}
