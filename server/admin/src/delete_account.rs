use lockbook_server_lib::{file_content_client, file_index_repo, ServerState};

pub async fn delete_account(mut server_state: ServerState, username: &str) -> bool {
    let mut transaction = server_state.index_db_client.begin().await.unwrap();

    // Ensure this is a real user
    file_index_repo::get_public_key(&mut transaction, username)
        .await
        .expect(&format!("Could not find public key for user {}", &username));

    file_index_repo::delete_account_access_keys(&mut transaction, &username)
        .await
        .expect("Failed to delete account access keys");

    file_index_repo::delete_all_files_of_account(&mut transaction, &username)
        .await
        .expect("Failed to delete all files of account")
        .responses;

    file_index_repo::delete_account(&mut transaction, &username)
        .await
        .expect("Failed to delete account");

    let files = file_index_repo::get_files(&mut transaction, &username)
        .await
        .expect("Failed to get files");

    transaction
        .commit()
        .await
        .expect("Failed to commit transaction");

    let mut ok = true;

    for file in files {
        if !file.is_folder {
            let problem = file_content_client::delete(
                &server_state.files_db_client,
                file.id,
                file.old_content_version,
            )
            .await
            .map_err(|err| {
                eprintln!(
                    "Failed to delete file in s3: {}, error: {:#?}",
                    file.id, err
                )
            })
            .is_err();

            if problem {
                ok = false;
            }
        }
    }

    ok
}
