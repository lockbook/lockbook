use lockbook_server_lib::{file_content_client, file_index_repo, ServerState};

pub async fn delete_account(mut server_state: ServerState, username: &str) -> bool {
    let transaction = server_state.index_db_client.transaction().await.unwrap();

    // Ensure this is a real user
    file_index_repo::get_public_key(&transaction, username)
        .await
        .expect(&format!("Could not find public key for user {}", &username));

    file_index_repo::delete_account_access_keys(&transaction, &username)
        .await
        .expect("Could not delete from user_access_keys.");

    file_index_repo::delete_account_from_usage_ledger(&transaction, &username)
        .await
        .expect("Could not delete from user_access_keys.");

    let deleted_files = file_index_repo::delete_all_files_of_account(&transaction, &username)
        .await
        .expect("Failed to delete root folder")
        .responses;

    file_index_repo::delete_account(&transaction, &username)
        .await
        .expect("Failed to delete account");

    transaction
        .commit()
        .await
        .expect("Failed to commit transaction");

    let mut ok = true;

    for file in deleted_files {
        if !file.is_folder {
            if file_content_client::delete(
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
            .is_err()
            {
                ok = false;
            }
        }
    }

    ok
}
