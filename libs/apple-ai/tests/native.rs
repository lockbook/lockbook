//! Explicit opt-in tests: require a ready local model and use synthetic text only.
use lb_apple_ai::{Event, Input, Message};
use std::time::Duration;

fn input(messages: Vec<Message>) -> Input {
    Input {
        tools: vec![],
        instructions: "Answer briefly and accurately using the conversation.".into(),
        messages,
        max_tokens: 128,
    }
}
fn user(text: &str) -> Message {
    Message { role: "user", content: text.into() }
}

async fn collect(input: Input) -> Result<String, String> {
    let mut request = lb_apple_ai::start(input)?;
    tokio::time::timeout(Duration::from_secs(60), async {
        let mut text = String::new();
        while let Some(event) = request.next().await {
            match event {
                Event::Delta(delta) => text.push_str(&delta),
                Event::Done => return Ok(text),
                Event::ToolCall(_) => panic!("Unexpected tool call"),
                Event::Error(error) => return Err(error),
            }
        }
        Err("Missing terminal event".into())
    })
    .await
    .map_err(|_| "Timed out".to_string())?
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires Apple Intelligence; synthetic local inference"]
async fn streams_restored_history_and_survives_cancellation() {
    std::thread::spawn(lb_apple_ai::availability)
        .join()
        .unwrap()
        .unwrap();
    let answer = collect(input(vec![
        user("The synthetic project code is 7319."),
        Message { role: "assistant", content: "I have noted the project code.".into() },
        user("What is the project code? Reply with the number."),
    ]))
    .await
    .unwrap();
    assert!(answer.contains("7319"), "restored history: {answer}");

    let mut cancelled =
        lb_apple_ai::start(input(vec![user("Write a long story about the ocean.")])).unwrap();
    let first = tokio::time::timeout(Duration::from_secs(60), cancelled.next())
        .await
        .unwrap();
    assert!(matches!(first, Some(Event::Delta(_))), "{first:?}");
    drop(cancelled);
    let answer = collect(input(vec![user("Say hello in one short sentence.")]))
        .await
        .unwrap();
    assert!(!answer.trim().is_empty(), "fresh request after Stop was empty");
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires Apple Intelligence; synthetic local inference"]
async fn oversized_context_returns_an_actionable_error() {
    let result = collect(input(vec![user(&"a long synthetic note. ".repeat(1000))])).await;
    let error = result.expect_err("large context should fail, never silently truncate");
    assert!(error.contains("context") || error.contains("too large"), "{error}");
}

fn tool_input() -> Input {
    let mut req = input(vec![user(
        "Read /juniper.md with read_note and tell me the project code. You must use the tool; do not guess.",
    )]);
    req.tools = vec![lb_apple_ai::Tool {
        name: "read_note".into(),
        description: "Read a note by its absolute path.".into(),
        parameters: serde_json::json!({"type":"object","properties":{"path":{"type":"string"}},"required":["path"],"additionalProperties":false}),
    }];
    req
}

#[tokio::test(flavor = "current_thread")]
#[ignore = "requires Apple Intelligence; synthetic tool result"]
async fn native_tool_round_trip_and_cancel_while_waiting() {
    tokio::time::timeout(Duration::from_secs(60), async {
        let mut request = lb_apple_ai::start(tool_input()).unwrap();
        let mut answer = String::new();
        let mut calls = 0;
        while let Some(event) = request.next().await {
            match event {
                Event::ToolCall(call) => {
                    calls += 1;
                    assert_eq!(call.name, "read_note");
                    assert_eq!(call.args["path"], "/juniper.md");
                    request
                        .tool_result(&call.id, "The project code is 943781.")
                        .unwrap();
                    // Duplicate and unknown results must not resume twice.
                    request.tool_result(&call.id, "duplicate").unwrap();
                    request.tool_result("unknown", "ignored").unwrap();
                }
                Event::Delta(text) => answer.push_str(&text),
                Event::Done => break,
                Event::Error(e) => panic!("{e}"),
            }
        }
        assert!(calls > 0);
        assert!(answer.contains("943781"), "{answer}");
        let mut cancelled = lb_apple_ai::start(tool_input()).unwrap();
        loop {
            match cancelled.next().await.unwrap() {
                Event::ToolCall(_) => break,
                Event::Delta(_) => {}
                other => panic!("expected tool call, got {other:?}"),
            }
        }
        drop(cancelled);
        let fresh = collect(input(vec![user("Say hello in one short sentence.")]))
            .await
            .unwrap();
        assert!(!fresh.trim().is_empty() && !fresh.contains("943781"), "fresh response: {fresh}");
    })
    .await
    .expect("tool session timed out");
}
