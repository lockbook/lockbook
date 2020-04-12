extern crate lockbook_core;
use lockbook_core::lockbook_api;
use lockbook_core::lockbook_api::CreateFileRequest;
use lockbook_core::lockbook_api::{FileMetadata, GetUpdatesRequest};
use lockbook_core::lockbook_api::NewAccountRequest;

#[macro_use]
pub mod utils;
use utils::{api_loc, generate_file_id, generate_username, TestError};

fn get_updates(username: String, file_id: String) -> Result<(Vec<FileMetadata>, u64), TestError> {
    lockbook_api::new_account(
        api_loc(),
        &NewAccountRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            pub_key_n: "test_pub_key_n".to_string(),
            pub_key_e: "test_pub_key_e".to_string(),
        },
    )?;

    let file_version = lockbook_api::create_file(
        api_loc(),
        &CreateFileRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content: "file_content".to_string(),
        },
    )?;

    let updates_metadata = lockbook_api::get_updates(
        api_loc(),
        &GetUpdatesRequest {
            username: username.to_string(),
            auth: "test_auth".to_string(),
            since_version: 0,
        },
    )?;

    Ok((updates_metadata, file_version))
}

#[test]
fn test_get_updates() {
    let username = generate_username();
    let file_id = generate_file_id();

    let updates_metadata_and_file_version = get_updates(username.to_string(), file_id.to_string());
    assert_matches!(&updates_metadata_and_file_version, &Ok(_));
    let (updates_metadata, file_version) = updates_metadata_and_file_version.unwrap();
    assert_eq!(
        updates_metadata[..],
        [FileMetadata {
            file_id: file_id.to_string(),
            file_name: "file_name".to_string(),
            file_path: "file_path".to_string(),
            file_content_version: file_version,
            file_metadata_version: file_version,
            deleted: false,
        }][..]
    );
}
