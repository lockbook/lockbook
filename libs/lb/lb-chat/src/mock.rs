//! A real socket serving canned responses, so tests exercise the full
//! reqwest and SSE path offline.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{Receiver, channel};

pub const SSE_HELLO: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
     data: {\"choices\":[{\"delta\":{\"content\":\"Hel\"}}]}\n\n\
     data: {\"choices\":[{\"delta\":{\"content\":\"lo\"}}]}\n\n\
     data: {\"choices\":[],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\
     \"prompt_tokens_details\":{\"cached_tokens\":3}}}\n\n\
     data: [DONE]\n\n";

/// An OpenAI-style streamed reply saying `text`, usage included.
pub fn sse_text(text: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
         data: {{\"choices\":[{{\"delta\":{{\"content\":{}}}}}]}}\n\n\
         data: {{\"choices\":[],\"usage\":{{\"prompt_tokens\":4,\"completion_tokens\":1}}}}\n\n\
         data: [DONE]\n\n",
        serde_json::to_string(text).unwrap()
    )
}

/// An OpenAI-style streamed reply that calls `name` with `args`.
pub fn sse_call(name: &str, args: &str) -> String {
    format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
         data: {{\"choices\":[{{\"delta\":{{\"tool_calls\":[{{\"index\":0,\"id\":\"c1\",\"function\":{{\"name\":{},\"arguments\":{}}}}}]}}}}]}}\n\n\
         data: [DONE]\n\n",
        serde_json::to_string(name).unwrap(),
        serde_json::to_string(args).unwrap()
    )
}

pub fn serve_once(response: &str) -> String {
    serve_seq(vec![response.to_string()])
}

/// Serves each response to one connection, in order, and hands back each
/// request body on the channel.
pub fn serve_seq(responses: Vec<String>) -> String {
    serve(responses).0
}

pub fn serve_capturing(response: &str) -> (String, Receiver<String>) {
    serve(vec![response.to_string()])
}

pub fn serve(responses: Vec<String>) -> (String, Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        for response in responses {
            let (mut sock, _) = listener.accept().unwrap();
            let body = read_request(&mut sock);
            let _ = tx.send(body);
            let _ = sock.write_all(response.as_bytes());
        }
    });
    (format!("http://{addr}"), rx)
}

/// Serves `response` in two writes split at `split_at`, a chunk boundary
/// landing wherever the caller aims it.
pub fn serve_split(response: &'static str, split_at: usize) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let (mut sock, _) = listener.accept().unwrap();
        read_request(&mut sock);
        let bytes = response.as_bytes();
        sock.write_all(&bytes[..split_at]).unwrap();
        sock.flush().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(50));
        sock.write_all(&bytes[split_at..]).unwrap();
    });
    format!("http://{addr}")
}

fn read_request(sock: &mut TcpStream) -> String {
    let mut buf = Vec::new();
    let mut tmp = [0u8; 4096];
    loop {
        let n = sock.read(&mut tmp).unwrap();
        buf.extend_from_slice(&tmp[..n]);
        let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else { continue };
        let headers = String::from_utf8_lossy(&buf[..end]).to_lowercase();
        let len = headers
            .lines()
            .find_map(|l| l.strip_prefix("content-length:"))
            .map_or(0, |v| v.trim().parse::<usize>().unwrap());
        if buf.len() >= end + 4 + len {
            return String::from_utf8_lossy(&buf[end + 4..end + 4 + len]).into_owned();
        }
    }
}
