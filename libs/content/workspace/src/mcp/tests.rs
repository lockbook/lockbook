use super::*;
use reqwest::blocking::{Client, Response};
use serde_json::json;

fn message(response: Response) -> Value {
    let body = response.text().unwrap();
    serde_json::from_str(&body).unwrap_or_else(|_| {
        body.lines()
            .filter_map(|line| line.strip_prefix("data: "))
            .find_map(|data| serde_json::from_str::<Value>(data).ok())
            .unwrap_or_else(|| panic!("No MCP JSON message: {body}"))
    })
}

#[test]
fn http_auth_origin_protocol_and_shutdown() {
    let port = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let token = "ab".repeat(32);
    let url = format!("http://127.0.0.1:{port}/mcp");
    configure(true, port, &token).unwrap();
    let client = Client::builder()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap();
    let request = json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{
        "protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"lockbook-test","version":"1"}
    }});
    let send = |auth: &str, origin: Option<&str>| {
        let mut req = client
            .post(&url)
            .header("Accept", "application/json, text/event-stream")
            .header("Authorization", auth)
            .json(&request);
        if let Some(origin) = origin {
            req = req.header("Origin", origin);
        }
        req.send().unwrap()
    };
    assert_eq!(send("Bearer wrong", None).status(), 401);
    assert_eq!(send(&format!("Bearer {token}"), Some("https://evil.example")).status(), 403);
    let init = send(&format!("Bearer {token}"), None);
    assert_eq!(init.status(), 200);
    let session = init
        .headers()
        .get("mcp-session-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let value = message(init);
    assert_eq!(value["result"]["serverInfo"]["name"], "lockbook-workspace");
    let call = |payload: Value| {
        client
            .post(&url)
            .header("Accept", "application/json, text/event-stream")
            .header("Authorization", format!("Bearer {token}"))
            .header("Mcp-Session-Id", &session)
            .header("MCP-Protocol-Version", "2025-03-26")
            .json(&payload)
            .send()
            .unwrap()
    };
    assert_eq!(call(json!({"jsonrpc":"2.0","method":"notifications/initialized"})).status(), 202);
    configure(true, port, &token).unwrap(); // Idempotent enable preserves the session.
    let tools = message(call(json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})));
    assert!(
        tools["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|t| t["name"] == "edit_document")
    );
    assert_eq!(tools["result"]["tools"].as_array().unwrap().len(), commands::tools().len());
    // Exercise actual transport-to-workspace command/reply delivery without an account.
    let consumer = std::thread::spawn(|| {
        let command = server()
            .lock()
            .unwrap()
            .as_ref()
            .unwrap()
            .rx
            .recv_timeout(Duration::from_secs(3))
            .unwrap();
        assert_eq!(command.name, "get_workspace");
        assert!(!command.reply.is_closed());
        command.reply.send(Ok(json!({"tabs": []}))).unwrap();
    });
    let response = message(call(
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_workspace","arguments":{}}}),
    ));
    consumer.join().unwrap();
    assert_eq!(response["result"]["structuredContent"]["tabs"], json!([]));
    let invalid = message(call(
        json!({"jsonrpc":"2.0","id":4,"method":"tools/call","params":{"name":"missing","arguments":{}}}),
    ));
    assert!(invalid.get("error").is_some());
    // Token reset rejects existing credentials, including old sessions.
    let new_token = "cd".repeat(32);
    configure(true, port, &new_token).unwrap();
    assert_eq!(send(&format!("Bearer {token}"), None).status(), 401);
    assert_eq!(send(&format!("Bearer {new_token}"), None).status(), 200);
    configure(false, 0, "").unwrap();
    assert!(client.post(&url).send().is_err());
    assert!(TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok());
}

#[test]
fn tokens_are_exact_and_case_sensitive() {
    assert!(token_matches(b"Bearer abc", b"Bearer abc"));
    assert!(!token_matches(b"Bearer Abc", b"Bearer abc"));
    assert!(!token_matches(b"Bearer abc ", b"Bearer abc"));
}

#[test]
fn create_retries_never_execute_twice_or_evict_protection() {
    let mut ledger = HashMap::new();
    let original = json!({"request_id": "one", "name": "note.md"});
    let result = json!({"id": "created-once"});
    assert_eq!(
        deduplicate_create(&mut ledger, &original, || Ok(result.clone())),
        Ok(result.clone())
    );
    assert_eq!(
        deduplicate_create(&mut ledger, &original, || panic!("duplicate create")),
        Ok(result)
    );
    assert!(
        deduplicate_create(
            &mut ledger,
            &json!({"request_id":"one","name":"different"}),
            || panic!("mismatched retry")
        )
        .is_err()
    );
    for i in 1..256 {
        deduplicate_create(&mut ledger, &json!({"request_id": i.to_string()}), || Ok(json!({})))
            .unwrap();
    }
    assert!(
        deduplicate_create(&mut ledger, &json!({"request_id":"overflow"}), || panic!(
            "ledger overflow"
        ))
        .is_err()
    );
    assert!(
        deduplicate_create(&mut ledger, &original, || panic!("evicted retry protection")).is_ok()
    );
}
