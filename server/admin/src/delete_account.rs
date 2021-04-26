use lockbook_models::api::GetRootRequest;
use lockbook_server_lib::{file_index_repo, RequestContext, ServerState};
use rsa::{BigUint, RSAPublicKey};

pub async fn delete_account(mut server_state: ServerState, username: &str) {
    let pub_key = {
        let transaction = server_state.index_db_client.transaction().await.unwrap();

        // Get account public key
        file_index_repo::get_public_key(&transaction, username)
            .await
            .expect(&format!("Could not find public key for user {}", &username))
    };

    let _request = RequestContext {
        server_state: &mut server_state,
        request: GetRootRequest {},
        public_key: pub_key,
    };

    // Find the user's root folder

    // Delete it

    // Delete anything that remains
}
