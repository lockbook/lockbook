//! xAI Realtime WebSocket. Live calls use server VAD.

use futures::{SinkExt, StreamExt};
use lb_rs::model::chat::ChatMsg;
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::header::AUTHORIZATION;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async};

use super::grok::BASE_URL;

pub const VOICE_MODEL: &str = "grok-voice-latest";

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub struct VoiceConn {
    ws: Ws,
}

pub fn realtime_url(base: &str, model: &str) -> String {
    let ws = if let Some(rest) = base.strip_prefix("https://") {
        format!("wss://{rest}")
    } else if let Some(rest) = base.strip_prefix("http://") {
        format!("ws://{rest}")
    } else {
        base.to_string()
    };
    let ws = ws.trim_end_matches('/');
    format!("{ws}/realtime?model={model}")
}

impl VoiceConn {
    pub async fn connect(bearer: &str, model: &str) -> Result<Self, String> {
        let url = realtime_url(BASE_URL, model);
        let mut req = url
            .as_str()
            .into_client_request()
            .map_err(|e| format!("voice request: {e}"))?;
        let auth = format!("Bearer {bearer}");
        req.headers_mut().insert(
            AUTHORIZATION,
            auth.parse()
                .map_err(|e| format!("voice auth header: {e}"))?,
        );
        let (ws, _resp) = connect_async(req)
            .await
            .map_err(|e| format!("voice connect: {e}"))?;
        Ok(Self { ws })
    }

    pub async fn send(&mut self, v: &Value) -> Result<(), String> {
        self.ws
            .send(Message::Text(v.to_string()))
            .await
            .map_err(|e| format!("voice send: {e}"))
    }

    pub async fn recv(&mut self) -> Result<Value, String> {
        loop {
            let msg = self
                .ws
                .next()
                .await
                .ok_or_else(|| "voice socket closed".to_string())?
                .map_err(|e| format!("voice recv: {e}"))?;
            match msg {
                Message::Text(t) => {
                    return serde_json::from_str(&t).map_err(|e| format!("voice json: {e}"));
                }
                Message::Ping(p) => {
                    let _ = self.ws.send(Message::Pong(p)).await;
                }
                Message::Close(c) => return Err(format!("voice closed: {c:?}")),
                Message::Pong(_) | Message::Binary(_) | Message::Frame(_) => {}
            }
        }
    }

    pub async fn wait_session_created(&mut self) -> Result<Value, String> {
        loop {
            let ev = self.recv().await?;
            if ev_type(&ev) == "session.created" {
                return Ok(ev);
            }
        }
    }

    pub async fn session_update(&mut self, session: Value) -> Result<(), String> {
        self.send(&json!({ "type": "session.update", "session": session }))
            .await
    }

    pub async fn append_pcm16(&mut self, pcm: &[u8]) -> Result<(), String> {
        self.send(&json!({
            "type": "input_audio_buffer.append",
            "audio": encode_b64(pcm),
        }))
        .await
    }

    pub async fn response_create(&mut self) -> Result<(), String> {
        self.send(&json!({ "type": "response.create" })).await
    }

    pub async fn response_cancel(&mut self) -> Result<(), String> {
        self.send(&json!({ "type": "response.cancel" })).await
    }

    pub async fn user_text(&mut self, text: &str) -> Result<(), String> {
        self.send(&json!({
            "type": "conversation.item.create",
            "item": {
                "type": "message",
                "role": "user",
                "content": [{ "type": "input_text", "text": text }]
            }
        }))
        .await
    }

    /// Seed a live socket from the local log so a call can pick up a typed chat.
    /// Skip tool results: `function_call_output` without a matching `function_call`
    /// on this socket is rejected.
    pub async fn seed_history(&mut self, msgs: &[ChatMsg]) -> Result<(), String> {
        for msg in msgs {
            match msg {
                ChatMsg::User { text, .. } if !text.trim().is_empty() => {
                    self.user_text(text).await?;
                }
                ChatMsg::Assistant { text, .. } if !text.trim().is_empty() => {
                    self.send(&json!({
                        "type": "conversation.item.create",
                        "item": {
                            "type": "message",
                            "role": "assistant",
                            "content": [{ "type": "output_text", "text": text }]
                        }
                    }))
                    .await?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub async fn function_output(&mut self, call_id: &str, output: &str) -> Result<(), String> {
        self.send(&json!({
            "type": "conversation.item.create",
            "item": {
                "type": "function_call_output",
                "call_id": call_id,
                "output": output,
            }
        }))
        .await
    }
}

pub fn ev_type(ev: &Value) -> &str {
    ev.get("type").and_then(|v| v.as_str()).unwrap_or("")
}

pub fn encode_b64(bytes: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let a = chunk[0] as u32;
        let b = chunk.get(1).copied().unwrap_or(0) as u32;
        let c = chunk.get(2).copied().unwrap_or(0) as u32;
        let n = (a << 16) | (b << 8) | c;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        if chunk.len() > 1 {
            out.push(T[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(T[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub fn decode_b64(s: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Result<u8, String> {
        match c {
            b'A'..=b'Z' => Ok(c - b'A'),
            b'a'..=b'z' => Ok(c - b'a' + 26),
            b'0'..=b'9' => Ok(c - b'0' + 52),
            b'+' => Ok(62),
            b'/' => Ok(63),
            _ => Err(format!("bad b64 byte {c}")),
        }
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    for chunk in bytes.chunks(4) {
        if chunk.len() < 4 {
            break;
        }
        let a = val(chunk[0])?;
        let b = val(chunk[1])?;
        let c = if chunk[2] == b'=' { 0 } else { val(chunk[2])? };
        let d = if chunk[3] == b'=' { 0 } else { val(chunk[3])? };
        out.push((a << 2) | (b >> 4));
        if chunk[2] != b'=' {
            out.push((b << 4) | (c >> 2));
        }
        if chunk[3] != b'=' {
            out.push((c << 6) | d);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{decode_b64, encode_b64, realtime_url};

    #[test]
    fn b64_roundtrip() {
        let src = b"\x00\x01\xff hello";
        assert_eq!(decode_b64(&encode_b64(src)).unwrap(), src);
    }

    #[test]
    fn realtime_url_wss() {
        assert_eq!(
            realtime_url("https://api.x.ai/v1", "grok-voice-latest"),
            "wss://api.x.ai/v1/realtime?model=grok-voice-latest"
        );
    }
}
