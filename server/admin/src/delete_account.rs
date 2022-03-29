use lockbook_models::api::DeleteAccountRequest;
use lockbook_server_lib::account_service::{public_key_from_username, purge_account};
use lockbook_server_lib::{RequestContext, ServerState};

pub async fn delete_account(server_state: ServerState, username: &str) -> bool {
    let public_key = public_key_from_username(username, &server_state)
        .await
        .unwrap_or_else(|_| panic!("Could not get public key for user: {}", username))
        .key;

    purge_account(RequestContext {
        server_state: &server_state,
        request: DeleteAccountRequest {},
        public_key,
    })
    .await
    .unwrap_or_else(|_| panic!("Could not get public key for user: {}", username));

    true
}
