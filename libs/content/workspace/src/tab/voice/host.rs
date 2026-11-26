use std::sync::{Arc, Mutex};

use egui::{Context, Id};
use lb_rs::Uuid;
use serde::{Deserialize, Serialize};

use super::format::Transcript;

#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Request {
    Record { id: Uuid },
    Stop { id: Uuid },
    Play { id: Uuid, path: String, from_ms: u64 },
    Pause { id: Uuid },
    Seek { id: Uuid, ms: u64 },
    Transcribe { id: Uuid, path: String },
    Release { id: Uuid },
}

#[derive(Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Preparing,
    Downloading,
    Transcribing,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    Recording { id: Uuid, elapsed_ms: u64, level: f32 },
    Recorded { id: Uuid, path: String, duration_ms: u64 },
    RecordFailed { id: Uuid, message: String },
    Playback { id: Uuid, position_ms: u64, playing: bool },
    PlaybackFailed { id: Uuid, message: String },
    Transcribing { id: Uuid, phase: Phase, fraction: f32, preview: Option<String> },
    Transcribed { id: Uuid, transcript: Transcript },
    TranscribeFailed { id: Uuid, message: String },
}

impl Event {
    pub fn id(&self) -> Uuid {
        match self {
            Event::Recording { id, .. }
            | Event::Recorded { id, .. }
            | Event::RecordFailed { id, .. }
            | Event::Playback { id, .. }
            | Event::PlaybackFailed { id, .. }
            | Event::Transcribing { id, .. }
            | Event::Transcribed { id, .. }
            | Event::TranscribeFailed { id, .. } => *id,
        }
    }
}

#[derive(Default)]
pub struct VoiceHost {
    requests: Mutex<Vec<Request>>,
    events: Mutex<Vec<Event>>,
}

impl VoiceHost {
    pub fn available() -> bool {
        cfg!(target_os = "ios")
    }

    pub fn request(&self, request: Request) {
        self.requests.lock().unwrap().push(request);
    }

    pub fn take_requests(&self) -> Vec<Request> {
        std::mem::take(&mut *self.requests.lock().unwrap())
    }

    pub fn post(&self, event: Event) {
        self.events.lock().unwrap().push(event);
    }

    pub fn post_json(&self, json: &str) -> Result<(), String> {
        let event: Event = serde_json::from_str(json).map_err(|e| e.to_string())?;
        self.post(event);
        Ok(())
    }

    pub fn take_events(&self, id: Uuid) -> Vec<Event> {
        let mut events = self.events.lock().unwrap();
        let (mine, rest) = std::mem::take(&mut *events)
            .into_iter()
            .partition(|e| e.id() == id);
        *events = rest;
        mine
    }
}

fn key() -> Id {
    Id::unique("lb_voice_host")
}

pub fn get(ctx: &Context) -> Arc<VoiceHost> {
    if let Some(host) = ctx.data(|d| d.get_temp::<Arc<VoiceHost>>(key())) {
        return host;
    }
    let host = Arc::new(VoiceHost::default());
    ctx.data_mut(|d| d.insert_temp(key(), host.clone()));
    host
}

pub fn requests_json(ctx: &Context) -> Option<String> {
    let requests = get(ctx).take_requests();
    if requests.is_empty() {
        return None;
    }
    serde_json::to_string(&requests).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_route_by_id() {
        let host = VoiceHost::default();
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();
        host.post_json(&format!(
            r#"{{"event":"recording","id":"{}","elapsed_ms":1200,"level":0.5}}"#,
            a.to_string().to_uppercase()
        ))
        .unwrap();
        host.post(Event::Playback { id: b, position_ms: 10, playing: true });
        assert_eq!(host.take_events(a).len(), 1);
        assert_eq!(host.take_events(a).len(), 0);
        assert_eq!(host.take_events(b).len(), 1);
    }

    #[test]
    fn transcribed_event_parses() {
        let id = Uuid::new_v4();
        let json = format!(
            r#"{{"event":"transcribed","id":"{id}","transcript":{{"engine":"apple","locale":"en_US","took_ms":5,"segments":[{{"words":[{{"text":"Hi","start_ms":0,"end_ms":300}}]}}]}}}}"#
        );
        let host = VoiceHost::default();
        host.post_json(&json).unwrap();
        assert!(
            matches!(&host.take_events(id)[0], Event::Transcribed { transcript, .. } if transcript.text() == "Hi")
        );
    }

    #[test]
    fn requests_serialize_with_op_tags() {
        let id = Uuid::nil();
        let json =
            serde_json::to_string(&Request::Play { id, path: "/t/a.m4a".into(), from_ms: 5 })
                .unwrap();
        assert_eq!(
            json,
            r#"{"op":"play","id":"00000000-0000-0000-0000-000000000000","path":"/t/a.m4a","from_ms":5}"#
        );
    }
}
