pub mod change_file_content;
pub mod create_file;
pub mod delete_file;
pub mod get_public_key;
pub mod get_updates;
pub mod move_file;
pub mod new_account;
pub mod rename_file;

pub fn username_is_valid(username: &str) -> bool {
    username.chars().all(|x| x.is_digit(36)) && username.to_lowercase() == *username
}
