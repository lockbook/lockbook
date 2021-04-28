pub fn username_is_valid(username: &str) -> bool {
    !username.is_empty()
        && username
            .to_lowercase()
            .chars()
            .all(|c| ('a'..='z').contains(&c) || ('0'..='9').contains(&c))
}
