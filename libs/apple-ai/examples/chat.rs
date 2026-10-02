//! Standalone smoke test: cargo run -p lb-apple-ai --example chat -- "Say hello"
use lb_apple_ai::{Event, Input, Message};
use std::io::Write;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    // Availability is synchronous; run before entering any blocking receive on
    // a Tokio thread by moving the check to a dedicated thread.
    std::thread::spawn(lb_apple_ai::availability)
        .join()
        .unwrap()?;
    let prompt = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let mut request = lb_apple_ai::start(Input {
        tools: vec![],
        instructions: "You are a helpful assistant. Be concise.".into(),
        messages: vec![Message {
            role: "user",
            content: if prompt.is_empty() { "Say hello in one sentence.".into() } else { prompt },
        }],
        max_tokens: 512,
    })?;
    while let Some(event) = request.next().await {
        match event {
            Event::Delta(text) => {
                print!("{text}");
                let _ = std::io::stdout().flush();
            }
            Event::Done => {
                println!();
                return Ok(());
            }
            Event::ToolCall(_) => return Err("Unexpected tool call".into()),
            Event::Error(e) => return Err(e),
        }
    }
    Err("Native stream closed unexpectedly".into())
}
