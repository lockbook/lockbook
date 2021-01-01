pub fn username_is_valid(username: &str) -> bool {
    !username.is_empty()
        && username
            .to_lowercase()
            .chars()
            .all(|c| ('a'..='z').contains(&c) || ('0'..='9').contains(&c))
}

pub fn version_is_supported(version: &str) -> bool {
    match version {
        "0.0.0" => false,
        "0.1.0" => true,
        "0.1.1" => true,
        "0.1.2" => true,
        _ => false,
    }
}
