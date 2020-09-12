pub fn username_is_valid(username: &str) -> bool {
    username
        .to_lowercase()
        .chars()
        .all(|c| (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9'))
}
