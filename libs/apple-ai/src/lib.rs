//! Small, on-device-only Apple Intelligence bridge. Swift never receives a Rust
//! allocation to retain/free: callbacks borrow UTF-8 bytes only for their duration.
//! Requests are identified by monotonically increasing IDs, not raw state pointers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

#[derive(Clone, Serialize)]
pub struct Message {
    pub role: &'static str,
    pub content: String,
}

#[derive(Serialize)]
pub struct Tool {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: serde_json::Value,
}

#[derive(Serialize)]
pub struct Input {
    pub instructions: String,
    pub messages: Vec<Message>,
    pub max_tokens: u32,
    pub tools: Vec<Tool>,
}

#[derive(Debug, PartialEq)]
pub enum Event {
    Delta(String),
    Done,
    ToolCall(ToolCall),
    Error(String),
}

type Registry = HashMap<u64, UnboundedSender<Event>>;
static REQUESTS: OnceLock<Mutex<Registry>> = OnceLock::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);
fn registry() -> &'static Mutex<Registry> {
    REQUESTS.get_or_init(Default::default)
}

pub struct Request {
    id: u64,
    events: UnboundedReceiver<Event>,
}

impl Request {
    fn register() -> Self {
        // Exhausting the ID space is preferable to reusing a live request ID.
        let id = NEXT_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .expect("Apple Intelligence request IDs exhausted");
        let (tx, events) = unbounded_channel();
        registry().lock().unwrap().insert(id, tx);
        Self { id, events }
    }

    /// Resolve only the named pending native tool call. Swift copies these bytes.
    pub fn tool_result(&self, call_id: &str, content: &str) -> Result<(), String> {
        let bytes = serde_json::to_vec(&serde_json::json!({"id": call_id, "content": content}))
            .map_err(|e| e.to_string())?;
        if bytes.len() > 128 * 1024 {
            return Err(
                "Tool result is too large for Apple Intelligence. Use a smaller note.".into()
            );
        }
        #[cfg(apple_ai_native)]
        unsafe {
            lb_apple_ai_tool_result(self.id, bytes.as_ptr(), bytes.len());
        }
        Ok(())
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.events.recv().await
    }

    /// Used by the existing background provider-discovery thread.
    pub fn next_blocking(&mut self) -> Option<Event> {
        self.events.blocking_recv()
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        // Unregister before asking Swift to cancel. Late events cannot reach UI.
        registry().lock().unwrap().remove(&self.id);
        #[cfg(apple_ai_native)]
        unsafe {
            lb_apple_ai_cancel(self.id)
        };
    }
}

fn deliver(id: u64, event: Event) {
    let terminal = matches!(event, Event::Done | Event::Error(_));
    let mut requests = registry().lock().unwrap();
    if let Some(tx) = requests.get(&id) {
        let _ = tx.send(event);
    }
    if terminal {
        requests.remove(&id);
    }
}

// Swift invokes this synchronously while the byte buffer is alive. No user code
// runs here. The panic boundary prevents unwinding across the C ABI.
#[cfg(apple_ai_native)]
extern "C" fn receive(id: u64, kind: u32, bytes: *const u8, len: usize) {
    let _ = std::panic::catch_unwind(|| {
        let text = if len == 0 {
            String::new()
        } else {
            if bytes.is_null() {
                return;
            }
            // SAFETY: private Swift bridge supplies a valid buffer for this call.
            String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(bytes, len) }).into_owned()
        };
        deliver(
            id,
            match kind {
                0 => Event::Delta(text),
                1 => Event::Done,
                3 => match serde_json::from_str(&text) {
                    Ok(call) => Event::ToolCall(call),
                    Err(_) => Event::Error("Invalid native tool call.".into()),
                },
                _ => Event::Error(text),
            },
        );
    });
}

#[cfg(apple_ai_native)]
unsafe extern "C" {
    fn lb_apple_ai_availability(id: u64, callback: extern "C" fn(u64, u32, *const u8, usize));
    fn lb_apple_ai_start(
        id: u64, bytes: *const u8, len: usize, callback: extern "C" fn(u64, u32, *const u8, usize),
    );
    fn lb_apple_ai_cancel(id: u64);
    fn lb_apple_ai_tool_result(id: u64, bytes: *const u8, len: usize);
}

pub fn availability() -> Result<(), String> {
    let mut request = Request::register();
    #[cfg(apple_ai_native)]
    unsafe {
        lb_apple_ai_availability(request.id, receive)
    };
    #[cfg(not(apple_ai_native))]
    deliver(request.id, Event::Error("Apple Intelligence requires an Apple Silicon Mac and a build made with Xcode 26 or newer. This build does not include native inference.".into()));
    match request.next_blocking() {
        Some(Event::Done) => Ok(()),
        Some(Event::Error(e)) => Err(e),
        _ => Err("Apple Intelligence did not report its availability.".into()),
    }
}

pub fn start(input: Input) -> Result<Request, String> {
    let bytes = serde_json::to_vec(&input).map_err(|e| e.to_string())?;
    // Bound the native decoder's input, including large pasted notes/history.
    if bytes.len() > 128 * 1024 {
        return Err("This conversation is too large for Apple Intelligence. Start a new chat or use a shorter note excerpt.".into());
    }
    let request = Request::register();
    #[cfg(apple_ai_native)]
    unsafe {
        lb_apple_ai_start(request.id, bytes.as_ptr(), bytes.len(), receive)
    };
    #[cfg(not(apple_ai_native))]
    deliver(request.id, Event::Error("Apple Intelligence is unavailable in this build. Use an Apple Silicon Mac built with Xcode 26 or newer.".into()));
    Ok(request)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dropped_request_cannot_deliver_to_a_new_turn() {
        let old = Request::register();
        let old_id = old.id;
        drop(old);
        let mut new = Request::register();
        assert_ne!(old_id, new.id);
        deliver(old_id, Event::Delta("stale".into()));
        assert!(new.events.try_recv().is_err());
        deliver(new.id, Event::Delta("current".into()));
        assert_eq!(new.events.try_recv().unwrap(), Event::Delta("current".into()));
    }

    #[test]
    fn terminal_event_closes_the_route_and_ignores_late_chunks() {
        let mut request = Request::register();
        deliver(request.id, Event::Done);
        deliver(request.id, Event::Delta("late".into()));
        assert_eq!(request.events.try_recv().unwrap(), Event::Done);
        assert!(request.events.try_recv().is_err());
    }
    #[test]
    fn tool_call_keeps_route_open_until_completion() {
        let mut request = Request::register();
        deliver(
            request.id,
            Event::ToolCall(ToolCall {
                id: "call".into(),
                name: "read_note".into(),
                args: serde_json::json!({"path":"/note.md"}),
            }),
        );
        assert!(matches!(request.events.try_recv(), Ok(Event::ToolCall(_))));
        deliver(request.id, Event::Delta("result received".into()));
        deliver(request.id, Event::Done);
        assert_eq!(request.events.try_recv().unwrap(), Event::Delta("result received".into()));
        assert_eq!(request.events.try_recv().unwrap(), Event::Done);
        assert!(request.events.try_recv().is_err());
    }
}
