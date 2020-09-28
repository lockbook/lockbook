pub fn username_is_valid(username: &str) -> bool {
    !username.is_empty()
        && username
            .to_lowercase()
            .chars()
            .all(|c| (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9'))
}

pub fn version_is_supported(version: &str) -> bool {
    match version {
        "0.1.2" => true,
        _ => false,
    }
}
