//! Live call on the same `.chat` log. Hangup leaves a typed-ready transcript;
//! starting a call seeds the socket from that log.

use std::collections::HashSet;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::{self, Receiver as StdReceiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use egui::Context;
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::chat::{
    Content, Event, EventBody, ItemKind, ItemMeta, SpeechPhase, Status, Transcript,
};
use serde_json::{Value, json};
use tracing::{error, info, warn};

use super::TabBridge;
use super::auth::{self, TokenSet};
use super::tools;

use crate::workspace::WsPersistentStore;

pub enum CallCmd {
    Hangup,
    Say(String),
    Barge,
    SetVoice(String),
}

pub enum CallEv {
    Log(Event),
    Listening { mic: String, spk: String },
    HungUp,
    Err(String),
}

/// Client-tool result, folded onto the voice loop like typed `tool_rx`.
struct ToolDone {
    item: Uuid,
    call_id: String,
    name: String,
    result: Result<String, String>,
}

pub fn supported() -> bool {
    cfg!(all(not(target_family = "wasm"), not(target_os = "android")))
}

pub struct CallSpawn {
    pub transcript: Transcript,
    pub from: String,
    pub tokens: TokenSet,
    pub cfg: WsPersistentStore,
    pub core: Lb,
    pub chat_id: Uuid,
    pub ctx: Context,
    pub instructions: String,
    pub voice: String,
    pub input: String,
    pub output: String,
    pub tabs: std::sync::Arc<std::sync::Mutex<TabBridge>>,
}

/// Returns the command sender. Events arrive on `ev_tx`.
pub fn start(opts: CallSpawn, ev_tx: Sender<CallEv>) -> Sender<CallCmd> {
    let (cmd_tx, cmd_rx) = mpsc::channel();
    #[cfg(all(not(target_family = "wasm"), not(target_os = "android")))]
    thread::spawn(move || {
        let ctx = opts.ctx.clone();
        let rt = match tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                let _ = ev_tx.send(CallEv::Err(e.to_string()));
                ctx.request_repaint();
                return;
            }
        };
        let result =
            catch_unwind(AssertUnwindSafe(|| rt.block_on(run(opts, cmd_rx, ev_tx.clone()))));
        match result {
            Ok(Ok(())) => {
                let _ = ev_tx.send(CallEv::HungUp);
            }
            Ok(Err(e)) => {
                error!(error = %e, "chat call failed");
                let _ = ev_tx.send(CallEv::Err(e));
            }
            Err(_) => {
                error!("chat call thread panicked");
                let _ = ev_tx.send(CallEv::Err("call crashed".into()));
            }
        }
        ctx.request_repaint();
    });
    #[cfg(not(all(not(target_family = "wasm"), not(target_os = "android"))))]
    {
        let _ = (opts, cmd_rx);
        let _ = ev_tx.send(CallEv::Err("voice is desktop-only".into()));
    }
    cmd_tx
}

const VAD_THRESHOLD: f64 = 0.5;
const VAD_PREFIX_MS: u32 = 300;
const VAD_SILENCE_MS: u32 = 200;

#[cfg(all(not(target_family = "wasm"), not(target_os = "android")))]
async fn run(
    opts: CallSpawn, cmd_rx: StdReceiver<CallCmd>, ev_tx: Sender<CallEv>,
) -> Result<(), String> {
    use super::audio::{self, Duplex};
    use super::realtime::{VOICE_MODEL, VoiceConn};

    let mut tokens = opts.tokens;
    let expires = tokens.expires_at;
    let bearer = auth::resolve_bearer(&mut tokens)?;
    if tokens.expires_at != expires {
        auth::save_prefs(&opts.cfg, &tokens);
    }
    let duplex = Duplex::open(&opts.input, &opts.output)?;
    info!(mic = %duplex.in_name, spk = %duplex.out_name, "chat call up");

    info!("chat call connecting");
    let mut ws = VoiceConn::connect(&bearer, VOICE_MODEL).await?;
    info!("chat call socket open");
    let _ = ws.wait_session_created().await?;
    info!("chat call session created");
    ws.session_update(session_body(&opts.instructions, &opts.voice))
        .await?;
    let _ =
        ev_tx.send(CallEv::Listening { mic: duplex.in_name.clone(), spk: duplex.out_name.clone() });
    opts.ctx.request_repaint();

    let (tool_tx, tool_rx) = mpsc::channel();
    let mut call = Call {
        transcript: opts.transcript,
        from: opts.from,
        last_ts: 0,
        ev_tx: ev_tx.clone(),
        ctx: opts.ctx.clone(),
        core: opts.core,
        chat_id: opts.chat_id,
        cfg: opts.cfg,
        tabs: opts.tabs,
        voice: opts.voice,
        user_id: None,
        asr_draft: String::new(),
        last_heard: None,
        pending_create: None,
        seen_calls: HashSet::new(),
        hung_up: false,
        tool_tx,
        muted_wires: HashSet::new(),
        held_audio: Vec::new(),
        held_wire: None,
        think_at: None,
        last_stall_warn: None,
    };
    if !call.transcript.items.is_empty() {
        let n = call.transcript.context().len();
        info!(n, "chat call seed history");
        ws.seed_history(&call.transcript.context()).await?;
    }

    let mut audio = Some(duplex);
    call.maybe_create(&mut ws).await?;

    loop {
        // Mic chunks arrive faster than the 8ms tick; Hangup must not wait
        // for that branch to win `select`.
        match cmd_rx.try_recv() {
            Ok(CallCmd::Hangup) | Err(std::sync::mpsc::TryRecvError::Disconnected) => break,
            Ok(CallCmd::Barge) => {
                call.barge_if_needed(&mut ws, audio.as_ref(), true).await?;
            }
            Ok(CallCmd::Say(text)) => {
                call.try_say(&mut ws, audio.as_ref(), text).await?;
            }
            Ok(CallCmd::SetVoice(v)) => {
                if !v.is_empty() {
                    if let Err(e) = ws.session_update(json!({ "voice": v })).await {
                        warn!(error = %e, "chat call voice update");
                    } else {
                        call.voice = v;
                    }
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {}
        }
        if call.hung_up {
            break;
        }
        while let Ok(done) = tool_rx.try_recv() {
            call.finish_tool(&mut ws, done).await?;
            if call.hung_up {
                break;
            }
        }
        if call.hung_up {
            break;
        }
        tokio::select! {
            chunk = recv_mic(&mut audio) => {
                let Some(pcm) = chunk else { break };
                if pcm.is_empty() {
                    continue;
                }
                let bytes = audio::i16_le_bytes(&pcm);
                if ws.append_pcm16(&bytes).await.is_err() {
                    break;
                }
            }
            ev = ws.recv() => {
                let ev = match ev {
                    Ok(v) => v,
                    Err(e) => {
                        warn!(error = %e, "chat call socket");
                        return Err(e);
                    }
                };
                call.on_ws(&ev, &mut ws, audio.as_mut()).await?;
                if call.hung_up {
                    break;
                }
                call.maybe_create(&mut ws).await?;
            }
            _ = tokio::time::sleep(Duration::from_millis(8)) => {
                call.stall_check();
            }
        }
    }
    if let Some(duplex) = audio.as_mut() {
        duplex.stop();
    }
    let q = audio.as_ref().map(|a| a.queue_len()).unwrap_or(0);
    call.hangup(q);
    drop(audio);
    Ok(())
}

#[cfg(all(not(target_family = "wasm"), not(target_os = "android")))]
async fn recv_mic(audio: &mut Option<super::audio::Duplex>) -> Option<Vec<i16>> {
    match audio {
        Some(a) => a.mic.recv().await,
        None => std::future::pending().await,
    }
}

#[cfg(all(not(target_family = "wasm"), not(target_os = "android")))]
fn session_body(instructions: &str, voice: &str) -> Value {
    let voice = if voice.is_empty() { tools::DEFAULT_VOICE } else { voice };
    json!({
        "voice": voice,
        "instructions": instructions,
        "modalities": ["audio", "text"],
        "input_audio_format": "pcm16",
        "output_audio_format": "pcm16",
        "input_audio_transcription": true,
        "turn_detection": {
            "type": "server_vad",
            "threshold": VAD_THRESHOLD,
            "prefix_padding_ms": VAD_PREFIX_MS,
            "silence_duration_ms": VAD_SILENCE_MS,
            "interrupt_response": false,
        },
        "tools": tools::call_tools(),
    })
}

#[cfg(all(not(target_family = "wasm"), not(target_os = "android")))]
struct Call {
    transcript: Transcript,
    from: String,
    last_ts: i64,
    ev_tx: Sender<CallEv>,
    ctx: Context,
    core: Lb,
    chat_id: Uuid,
    cfg: WsPersistentStore,
    tabs: std::sync::Arc<std::sync::Mutex<TabBridge>>,
    voice: String,
    user_id: Option<Uuid>,
    asr_draft: String,
    last_heard: Option<Uuid>,
    pending_create: Option<Uuid>,
    seen_calls: HashSet<String>,
    hung_up: bool,
    tool_tx: Sender<ToolDone>,
    /// Response ids we sent `response.cancel` for. Leftover deltas stay silent.
    muted_wires: HashSet<String>,
    /// PCM held while server VAD still has the user floor. The server often
    /// starts a reply before `speech_stopped`; we must not play over you, and
    /// we must not cancel that reply (that was dead air).
    held_audio: Vec<u8>,
    held_wire: Option<String>,
    /// First empty in-flight assistant, for stall warnings.
    think_at: Option<Instant>,
    last_stall_warn: Option<Instant>,
}

#[cfg(all(not(target_family = "wasm"), not(target_os = "android")))]
impl Call {
    fn emit(&mut self, body: EventBody) {
        let now = chrono::Utc::now().timestamp_millis();
        let ts = now.max(self.last_ts + 1);
        self.last_ts = ts;
        let ev = Event::new(self.from.clone(), ts, body);
        self.transcript.apply(&ev);
        let _ = self.ev_tx.send(CallEv::Log(ev));
        self.ctx.request_repaint();
    }

    fn n_thinking(&self) -> usize {
        self.transcript
            .items
            .iter()
            .filter(|i| i.kind == ItemKind::Assistant && i.status.in_flight() && !i.has_text())
            .count()
    }

    fn note_thinking(&mut self) {
        let n = self.n_thinking();
        if n >= 2 {
            warn!(n, "chat call duplicate thinking");
        }
        if n > 0 && self.think_at.is_none() {
            self.think_at = Some(Instant::now());
        }
        if n == 0 {
            self.think_at = None;
        }
    }

    fn clear_thinking_if_idle(&mut self) {
        if self.n_thinking() == 0 {
            self.think_at = None;
        }
    }

    fn stall_check(&mut self) {
        let Some(t0) = self.think_at else {
            return;
        };
        let elapsed = t0.elapsed();
        if elapsed < Duration::from_secs(5) {
            return;
        }
        let due = self
            .last_stall_warn
            .map(|t| t.elapsed() >= Duration::from_secs(5))
            .unwrap_or(true);
        if !due {
            return;
        }
        self.last_stall_warn = Some(Instant::now());
        warn!(
            elapsed_ms = elapsed.as_millis() as u64,
            think = self.n_thinking(),
            user = ?self.user_id,
            floor = self.transcript.user_floor.is_some(),
            pending = self.pending_create.is_some(),
            on_turn = self.transcript.on_turn_assistant().is_some(),
            asr = self.asr_draft.len(),
            follow_up = self.transcript.tools_awaiting_follow_up(),
            "chat call thinking stalled"
        );
    }

    fn log_ws(&self, ev: &Value) {
        use super::realtime::ev_type;
        let ty = ev_type(ev);
        if ty.ends_with(".delta") || ty == "input_audio_buffer.append" {
            return;
        }
        info!(
            ty,
            item = item_id(ev).unwrap_or(""),
            response = response_id(ev).unwrap_or(""),
            user = ?self.user_id,
            floor = self.transcript.user_floor.is_some(),
            pending = self.pending_create.is_some(),
            think = self.n_thinking(),
            on_turn = self.transcript.on_turn_assistant().is_some(),
            asr = self.asr_draft.len(),
            "chat call ws"
        );
    }

    fn stamp_wire(&mut self, item: Uuid, wire: &str) {
        if wire.is_empty() {
            return;
        }
        if self
            .transcript
            .by_id(item)
            .and_then(|i| i.meta.wire_id.as_deref())
            == Some(wire)
        {
            return;
        }
        self.emit(EventBody::SetMeta {
            item,
            meta: ItemMeta { wire_id: Some(wire.to_string()), ..ItemMeta::default() },
        });
    }

    /// Server VAD owns unanswered user turns. We only `response.create` after
    /// a client tool finishes (and after typed-into-call `try_say`).
    async fn maybe_create(&mut self, ws: &mut super::realtime::VoiceConn) -> Result<(), String> {
        if self.transcript.user_floor.is_some() || self.transcript.on_turn_assistant().is_some() {
            return Ok(());
        }
        if !self.transcript.tools_awaiting_follow_up() {
            return Ok(());
        }
        self.create_response(ws).await
    }

    async fn create_response(&mut self, ws: &mut super::realtime::VoiceConn) -> Result<(), String> {
        if self.transcript.on_turn_assistant().is_some() {
            return Ok(());
        }
        let item = Uuid::new_v4();
        let parent = self.transcript.items.last().map(|i| i.id);
        self.emit(EventBody::Open { item, parent, kind: ItemKind::Assistant });
        self.emit(EventBody::SetStatus { item, status: Status::Running });
        self.pending_create = Some(item);
        self.note_thinking();
        info!(%item, think = self.n_thinking(), "chat call response.create");
        ws.response_create().await
    }

    async fn try_say(
        &mut self, ws: &mut super::realtime::VoiceConn, audio: Option<&super::audio::Duplex>,
        text: String,
    ) -> Result<(), String> {
        let text = text.trim().to_string();
        if text.is_empty() {
            return Ok(());
        }
        self.barge_if_needed(ws, audio, true).await?;
        if self.user_id.is_some() {
            self.close_user_item();
        }
        let item = Uuid::new_v4();
        let parent = self.transcript.items.last().map(|i| i.id);
        self.emit(EventBody::Open { item, parent, kind: ItemKind::User });
        self.emit(EventBody::Replace { item, blocks: vec![Content::Text { text: text.clone() }] });
        self.emit(EventBody::SetStatus { item, status: Status::Done });
        self.last_heard = Some(item);
        info!(chars = text.len(), "chat call say");
        ws.user_text(&text).await?;
        // `user_text` does not auto-respond; this is not a VAD substitute.
        self.create_response(ws).await
    }

    async fn user_began(
        &mut self, ws: &mut super::realtime::VoiceConn, audio: &super::audio::Duplex, wire: &str,
    ) -> Result<(), String> {
        // Barge only if the assistant is actually speaking, or a client tool
        // is running. Idle Thinking is not a lock and must not hide this
        // utterance — VAD owns the reply; we still bind the user row.
        let begun = reply_has_begun(
            &self.in_flight_asst_text(),
            audio.queue_len(),
            !self.held_audio.is_empty(),
        );
        if begun || self.tools_running() {
            self.barge_if_needed(ws, Some(audio), false).await?;
        } else if self.transcript.on_turn_assistant().is_some() {
            info!(
                think = self.n_thinking(),
                queue = audio.queue_len(),
                wire,
                "chat call vad during pending reply"
            );
        }
        self.bind_live_user(wire);
        Ok(())
    }

    fn bind_live_user(&mut self, wire: &str) {
        if !wire.is_empty() {
            if let Some(id) = self.transcript.by_wire(wire) {
                if self.transcript.by_id(id).map(|i| i.kind) == Some(ItemKind::User) {
                    if self.user_id.is_some() && self.user_id != Some(id) {
                        self.close_user_item();
                    }
                    self.take_user(id, wire);
                    info!(%id, wire, "chat call vad reuse wire");
                    return;
                }
            }
        }
        if let Some(id) = self.user_id {
            let existing = self
                .transcript
                .by_id(id)
                .and_then(|i| i.meta.wire_id.clone());
            if let (Some(old), true) = (existing.as_deref(), !wire.is_empty()) {
                if old != wire {
                    self.close_user_item();
                } else {
                    self.take_user(id, wire);
                    info!(%id, wire, "chat call vad continue");
                    return;
                }
            } else {
                self.take_user(id, wire);
                info!(%id, wire, "chat call vad already on user");
                return;
            }
        }
        if let Some(id) = self.resume_user() {
            self.take_user(id, wire);
            info!(%id, wire, "chat call vad resume empty user");
            return;
        }
        let item = Uuid::new_v4();
        let parent = self.transcript.items.last().map(|i| i.id);
        self.emit(EventBody::Open { item, parent, kind: ItemKind::User });
        self.emit(EventBody::Speech { item, phase: SpeechPhase::Start });
        self.stamp_wire(item, wire);
        self.user_id = Some(item);
        self.asr_draft.clear();
        info!(%item, wire, "chat call vad start");
    }

    fn take_user(&mut self, item: Uuid, wire: &str) {
        self.stamp_wire(item, wire);
        if self.transcript.user_floor != Some(item) {
            self.emit(EventBody::Speech { item, phase: SpeechPhase::Start });
        }
        if self.transcript.by_id(item).map(|i| i.status) != Some(Status::Open) {
            self.emit(EventBody::SetStatus { item, status: Status::Open });
        }
        self.user_id = Some(item);
        if self.asr_draft.is_empty() {
            self.asr_draft = self
                .transcript
                .by_id(item)
                .map(|i| i.text())
                .unwrap_or_default();
        }
    }

    fn resume_user(&self) -> Option<Uuid> {
        let id = self.last_heard?;
        let item = self.transcript.by_id(id)?;
        if item.kind == ItemKind::User && !item.has_text() && item.meta.wire_id.is_none() {
            Some(id)
        } else {
            None
        }
    }

    fn close_user_item(&mut self) {
        let Some(item) = self.user_id.take() else {
            return;
        };
        if self.transcript.user_floor == Some(item) {
            self.emit(EventBody::Speech { item, phase: SpeechPhase::Stop });
        }
        if !self.asr_draft.is_empty() {
            self.emit(EventBody::Replace {
                item,
                blocks: vec![Content::Text { text: self.asr_draft.clone() }],
            });
        }
        self.emit(EventBody::SetStatus { item, status: Status::Done });
        self.last_heard = Some(item);
        info!(%item, chars = self.asr_draft.len(), "chat call user close");
        self.asr_draft.clear();
    }

    fn speech_stopped(&mut self) {
        let Some(item) = self.user_id else {
            return;
        };
        if self.transcript.user_floor == Some(item) {
            self.emit(EventBody::Speech { item, phase: SpeechPhase::Stop });
        }
        let has_text = !self.asr_draft.is_empty()
            || self
                .transcript
                .by_id(item)
                .map(|i| i.has_text())
                .unwrap_or(false);
        if has_text {
            self.close_user_item();
        } else {
            // VAD often ends before ASR. Leave the row Open so later events
            // with this item_id update the same UUID. Do not drop replies.
            info!(%item, "chat call speech_stopped waiting asr");
        }
    }

    fn apply_asr(&mut self, item: Uuid, text: String, completed: bool) {
        if text.is_empty() {
            if completed {
                self.finish_user_if_silent(item);
            }
            return;
        }
        let prev = self
            .transcript
            .by_id(item)
            .map(|i| i.text())
            .unwrap_or_default();
        if self.user_id == Some(item) {
            self.asr_draft = text.clone();
        } else if self.user_id.is_some() {
            info!(
                target = %item,
                user = ?self.user_id,
                last = ?self.last_heard,
                chars = text.len(),
                "chat call asr on other item"
            );
        }
        if prev != text {
            self.emit(EventBody::Replace { item, blocks: vec![Content::Text { text }] });
        }
        if completed {
            self.finish_user_if_silent(item);
        }
    }

    fn finish_user_if_silent(&mut self, item: Uuid) {
        if self.transcript.user_floor == Some(item) {
            return;
        }
        let Some(it) = self.transcript.by_id(item) else {
            return;
        };
        if it.kind != ItemKind::User || !it.status.in_flight() {
            return;
        }
        let chars = it.text().len();
        self.emit(EventBody::SetStatus { item, status: Status::Done });
        self.last_heard = Some(item);
        if self.user_id == Some(item) {
            self.user_id = None;
            self.asr_draft.clear();
        }
        info!(%item, chars, "chat call user asr done");
    }

    fn in_flight_asst_text(&self) -> String {
        self.transcript
            .items
            .iter()
            .find(|i| i.kind == ItemKind::Assistant && i.status.in_flight() && i.has_text())
            .map(|i| i.text())
            .unwrap_or_default()
    }

    fn bind_assistant(&mut self, wire: &str) -> Option<Uuid> {
        if !wire.is_empty() {
            if let Some(id) = self.transcript.by_wire(wire) {
                if let Some(pending) = self.pending_create {
                    if pending != id {
                        self.emit(EventBody::SetStatus {
                            item: pending,
                            status: Status::Cancelled,
                        });
                        info!(%pending, %id, wire, "chat call drop duplicate pending");
                    }
                    self.pending_create = None;
                }
                return Some(id);
            }
        }
        if let Some(item) = self.pending_create.take() {
            self.stamp_wire(item, wire);
            info!(%item, wire, "chat call response bind pending");
            return Some(item);
        }
        if let Some(item) = self.transcript.on_turn_assistant().filter(|&id| {
            self.transcript
                .by_id(id)
                .and_then(|i| i.meta.wire_id.as_deref())
                .is_none()
        }) {
            self.stamp_wire(item, wire);
            info!(%item, wire, "chat call response bind on_turn");
            return Some(item);
        }
        if wire.is_empty() {
            warn!("chat call response missing id");
            return None;
        }
        let item = Uuid::new_v4();
        let parent = self.transcript.items.last().map(|i| i.id);
        self.emit(EventBody::Open { item, parent, kind: ItemKind::Assistant });
        self.stamp_wire(item, wire);
        self.emit(EventBody::SetStatus { item, status: Status::Running });
        self.note_thinking();
        info!(%item, wire, "chat call response created");
        Some(item)
    }

    fn live_assistant(&mut self, wire: &str) -> Option<Uuid> {
        if wire.is_empty() || self.muted_wires.contains(wire) {
            return None;
        }
        let id = self.bind_assistant(wire)?;
        let live = self
            .transcript
            .by_id(id)
            .map(|i| i.kind == ItemKind::Assistant && i.status.in_flight())
            .unwrap_or(false);
        if live { Some(id) } else { None }
    }

    fn cancel_assistant(&mut self, item: Uuid) {
        if let Some(wire) = self
            .transcript
            .by_id(item)
            .and_then(|i| i.meta.wire_id.clone())
        {
            self.muted_wires.insert(wire);
        }
        if self.pending_create == Some(item) {
            self.pending_create = None;
        }
        self.drop_held();
        self.emit(EventBody::SetStatus { item, status: Status::Cancelled });
    }

    /// Server VAD (`speech_started` … `speech_stopped`) is how we know you're
    /// talking. Waiting on ASR (`user_id` with the floor cleared) is not.
    fn user_is_talking(&self) -> bool {
        self.transcript.user_floor.is_some()
    }

    fn drop_held(&mut self) {
        self.held_audio.clear();
        self.held_wire = None;
    }

    fn release_held(&mut self, audio: Option<&super::audio::Duplex>) {
        if self.held_audio.is_empty() {
            self.held_wire = None;
            return;
        }
        if let Some(audio) = audio {
            audio.push_voice_pcm16(&self.held_audio);
        }
        self.drop_held();
    }

    /// Queue PCM, or hold it while the user floor is up. Returns true when the
    /// hold overflowed — the caller should cancel that response.
    fn enqueue_voice(
        &mut self, audio: Option<&super::audio::Duplex>, wire: &str, bytes: &[u8],
    ) -> bool {
        if bytes.is_empty() || wire.is_empty() || self.muted_wires.contains(wire) {
            return false;
        }
        if self.user_is_talking() {
            if self.held_wire.as_deref() != Some(wire) {
                self.held_audio.clear();
                self.held_wire = Some(wire.to_string());
            }
            self.held_audio.extend_from_slice(bytes);
            // Late `speech_stopped` is hundreds of ms. A full second of hold
            // means you're talking over a reply — drop it, don't dump a backlog.
            if self.held_audio.len() > HOLD_AUDIO_CAP {
                warn!(
                    wire,
                    held = self.held_audio.len(),
                    "chat call drop held audio: talking over"
                );
                self.drop_held();
                self.muted_wires.insert(wire.to_string());
                return true;
            }
            return false;
        }
        let Some(audio) = audio else {
            return false;
        };
        if self.held_wire.as_deref() == Some(wire) && !self.held_audio.is_empty() {
            audio.push_voice_pcm16(&self.held_audio);
            self.drop_held();
        }
        audio.push_voice_pcm16(bytes);
        false
    }

    async fn barge_if_needed(
        &mut self, ws: &mut super::realtime::VoiceConn, audio: Option<&super::audio::Duplex>,
        force: bool,
    ) -> Result<(), String> {
        let queue = audio.map(|a| a.queue_len()).unwrap_or(0);
        let begun =
            reply_has_begun(&self.in_flight_asst_text(), queue, !self.held_audio.is_empty());
        if !force && !begun && !self.tools_running() {
            return Ok(());
        }
        if let Some(audio) = audio {
            if queue > 0 {
                audio.barge_in();
            }
        }
        self.drop_held();
        let tools = self.tools_running();
        let targets: Vec<Uuid> = self
            .transcript
            .items
            .iter()
            .filter(|i| i.kind == ItemKind::Assistant && i.status.in_flight())
            .filter(|i| force || tools || i.has_text())
            .map(|i| i.id)
            .collect();
        if targets.is_empty() {
            return Ok(());
        }
        let _ = ws.response_cancel().await;
        for item in targets {
            let chars = self
                .transcript
                .by_id(item)
                .map(|i| i.text().len())
                .unwrap_or(0);
            info!(%item, chars, force, "chat call barge");
            self.cancel_assistant(item);
        }
        self.clear_thinking_if_idle();
        Ok(())
    }

    fn hangup(&mut self, _queue: usize) {
        if self.hung_up {
            return;
        }
        self.hung_up = true;
        self.drop_held();
        if self.user_id.is_some() {
            self.close_user_item();
        }
        let stale: Vec<Uuid> = self
            .transcript
            .items
            .iter()
            .filter(|i| i.status.in_flight())
            .map(|i| i.id)
            .collect();
        for item in stale {
            self.emit(EventBody::SetStatus { item, status: Status::Cancelled });
        }
        info!(think = self.n_thinking(), "chat call hangup");
    }

    async fn on_ws(
        &mut self, ev: &Value, ws: &mut super::realtime::VoiceConn,
        audio: Option<&mut super::audio::Duplex>,
    ) -> Result<(), String> {
        use super::realtime::{decode_b64, ev_type};
        self.log_ws(ev);
        match ev_type(ev) {
            "input_audio_buffer.speech_started" => {
                let wire = item_id(ev).unwrap_or("");
                if let Some(duplex) = audio.as_deref() {
                    self.user_began(ws, duplex, wire).await?;
                } else {
                    self.bind_live_user(wire);
                }
            }
            "input_audio_buffer.speech_stopped" => {
                self.speech_stopped();
                self.release_held(audio.as_deref());
            }
            "response.created" => {
                let wire = response_id(ev).unwrap_or("");
                if self.muted_wires.contains(wire) {
                    info!(wire, "chat call response.created after cancel");
                } else {
                    let _ = self.bind_assistant(wire);
                }
            }
            "response.output_audio.delta" | "response.audio.delta" => {
                let wire = response_id(ev).unwrap_or("");
                if self.live_assistant(wire).is_some() {
                    if let Some(b64) = ev.get("delta").and_then(|v| v.as_str()) {
                        if let Ok(bytes) = decode_b64(b64) {
                            if self.enqueue_voice(audio.as_deref(), wire, &bytes) {
                                if let Some(duplex) = audio.as_deref() {
                                    if duplex.queue_len() > 0 {
                                        duplex.barge_in();
                                    }
                                }
                                let _ = ws.response_cancel().await;
                                if let Some(id) = self.transcript.by_wire(wire) {
                                    self.cancel_assistant(id);
                                }
                                info!(wire, "chat call barge held overflow");
                            }
                        }
                    }
                }
            }
            "response.output_audio_transcript.delta" | "response.output_text.delta" => {
                let wire = response_id(ev).unwrap_or("");
                if let (Some(item), Some(d)) =
                    (self.live_assistant(wire), ev.get("delta").and_then(|v| v.as_str()))
                {
                    if !d.is_empty() {
                        let mut text = self
                            .transcript
                            .by_id(item)
                            .map(|i| i.text())
                            .unwrap_or_default();
                        text.push_str(d);
                        self.emit(EventBody::Replace {
                            item,
                            blocks: vec![Content::Text { text }],
                        });
                        self.clear_thinking_if_idle();
                    }
                }
            }
            "response.done" => {
                let Some(wire) = response_id(ev) else {
                    return Ok(());
                };
                if self.muted_wires.contains(wire) {
                    return Ok(());
                }
                let Some(item) = self.transcript.by_wire(wire) else {
                    warn!(wire, "chat call response.done unmatched");
                    return Ok(());
                };
                let Some(it) = self.transcript.by_id(item) else {
                    return Ok(());
                };
                if !it.status.in_flight() {
                    info!(%item, wire, status = ?it.status, "chat call response.done not in-flight");
                    return Ok(());
                }
                self.emit(EventBody::SetStatus { item, status: Status::Done });
                self.clear_thinking_if_idle();
            }
            "response.function_call_arguments.done" => {
                if let Some(wire) = response_id(ev) {
                    if self.muted_wires.contains(wire) {
                        info!(wire, "chat call drop function_call: cancelled");
                        return Ok(());
                    }
                    let _ = self.bind_assistant(wire);
                }
                self.handle_tool(ev, ws, audio).await?;
            }
            "conversation.item.input_audio_transcription.updated"
            | "conversation.item.input_audio_transcription.completed" => {
                let completed = ev_type(ev).ends_with("completed");
                if let Some(text) = transcript_text(ev) {
                    let id = item_id(ev)
                        .and_then(|w| self.transcript.by_wire(w))
                        .or(self.user_id)
                        .or(self.last_heard);
                    if let Some(id) = id {
                        self.apply_asr(id, text, completed);
                    } else if completed {
                        warn!(
                            wire = item_id(ev).unwrap_or(""),
                            chars = text.len(),
                            "chat call asr with no item"
                        );
                    }
                }
            }
            "error" => {
                let summary = error_summary(ev);
                if summary.contains("Cancellation failed: no active response") {
                    return Ok(());
                }
                warn!(error = %summary, "chat call ws error");
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_tool(
        &mut self, ev: &Value, ws: &mut super::realtime::VoiceConn,
        audio: Option<&mut super::audio::Duplex>,
    ) -> Result<(), String> {
        let Some((call_id, name, args)) = tools::parse_call(ev) else {
            return Ok(());
        };
        if !self.seen_calls.insert(call_id.clone()) {
            return Ok(());
        }
        if !tools::is_client(&name) && !tools::is_server(&name) {
            return Ok(());
        }
        let parent = response_id(ev)
            .and_then(|w| self.transcript.by_wire(w))
            .or_else(|| self.transcript.on_turn_assistant())
            .or_else(|| self.transcript.last_assistant());
        let item = Uuid::new_v4();
        self.emit(EventBody::Open { item, parent, kind: ItemKind::Tool });
        self.emit(EventBody::SetMeta {
            item,
            meta: ItemMeta {
                title: Some(tools::summary(&name, &args)),
                tool_kind: Some(name.clone()),
                wire_id: Some(call_id.clone()),
                args: Some(args.clone()),
            },
        });
        if tools::is_server(&name) {
            let body = tools::persist_body(&name, &args);
            self.emit(EventBody::Replace { item, blocks: vec![Content::Text { text: body }] });
            self.emit(EventBody::SetStatus { item, status: Status::Done });
            return Ok(());
        }
        self.emit(EventBody::SetStatus { item, status: Status::Running });
        if name == "hangup" {
            return self
                .finish_tool(ws, ToolDone { item, call_id, name, result: Ok("hanging up".into()) })
                .await;
        }
        if name == "settings" {
            let result = self.apply_settings(ws, audio, &args).await;
            return self
                .finish_tool(ws, ToolDone { item, call_id, name, result })
                .await;
        }
        // Same fold as typed `spawn_tools`: work off the voice loop so mic,
        // barge, and hangup keep moving. Result lands on `tool_rx`.
        info!(name, %item, "chat call tool spawn");
        let core = self.core.clone();
        let chat_id = self.chat_id;
        let name_run = name.clone();
        let args_run = args.clone();
        let tabs = self.tabs.clone();
        let cfg = self.cfg.clone();
        let tx = self.tool_tx.clone();
        let ctx = self.ctx.clone();
        tokio::task::spawn_blocking(move || {
            let result = if name_run == "tabs" {
                let snaps = tabs.lock().unwrap().snaps.clone();
                match tools::tabs(&core, chat_id, &args_run, &snaps) {
                    Ok((msg, ops)) => {
                        if !ops.is_empty() {
                            tabs.lock().unwrap().cmds.extend(ops);
                        }
                        Ok(msg)
                    }
                    Err(e) => Err(e),
                }
            } else {
                super::dispatch_client_tool(&core, chat_id, &name_run, &args_run, &tabs, &cfg, true)
                    .map(|o| o.text)
            };
            let _ = tx.send(ToolDone { item, call_id, name: name_run, result });
            ctx.request_repaint();
        });
        Ok(())
    }

    async fn finish_tool(
        &mut self, ws: &mut super::realtime::VoiceConn, done: ToolDone,
    ) -> Result<(), String> {
        let ToolDone { item, call_id, name, result } = done;
        let (text, status) = match result {
            Ok(t) => (t, Status::Done),
            Err(e) => (e, Status::Failed),
        };
        info!(name, %item, status = ?status, "chat call tool done");
        self.emit(EventBody::Replace { item, blocks: vec![Content::Text { text: text.clone() }] });
        self.emit(EventBody::SetStatus { item, status });
        ws.function_output(&call_id, &text).await?;
        if name == "hangup" {
            self.hung_up = true;
            return Ok(());
        }
        self.maybe_create(ws).await
    }

    async fn apply_settings(
        &mut self, ws: &mut super::realtime::VoiceConn, audio: Option<&mut super::audio::Duplex>,
        args: &Value,
    ) -> Result<String, String> {
        let grok = self.cfg.grok();
        let mut state = tools::SettingsState {
            voice: if self.voice.is_empty() {
                tools::DEFAULT_VOICE.to_string()
            } else {
                self.voice.clone()
            },
            input: audio
                .as_ref()
                .map(|d| d.in_name.clone())
                .unwrap_or(grok.input),
            output: audio
                .as_ref()
                .map(|d| d.out_name.clone())
                .unwrap_or(grok.output),
        };
        let kind = args.get("kind").and_then(Value::as_str).unwrap_or("");
        let spec = args.get("name").and_then(Value::as_str);
        // Live device switch uses the duplex when we're on a call so the
        // stream actually moves; voice always goes through session.update.
        if matches!(kind, "input" | "output") {
            if let (Some(duplex), Some(s)) = (audio, spec.filter(|s| !s.trim().is_empty())) {
                let msg =
                    if kind == "input" { duplex.set_input(s)? } else { duplex.set_output(s)? };
                let mut grok = self.cfg.grok();
                grok.input = duplex.in_name.clone();
                grok.output = duplex.out_name.clone();
                self.cfg.set_grok(grok);
                return Ok(msg);
            }
        }
        let msg = tools::settings(args, &mut state)?;
        if kind == "voice" && state.voice != self.voice {
            ws.session_update(json!({ "voice": state.voice })).await?;
            self.voice = state.voice.clone();
        }
        let mut grok = self.cfg.grok();
        grok.voice = state.voice;
        grok.input = state.input;
        grok.output = state.output;
        self.cfg.set_grok(grok);
        Ok(msg)
    }

    fn tools_running(&self) -> bool {
        self.transcript
            .items
            .iter()
            .any(|i| i.kind == ItemKind::Tool && i.status == Status::Running)
    }
}

/// pcm16 @ 24kHz, ~1s. Longer than late `speech_stopped`; shorter than talking over.
const HOLD_AUDIO_CAP: usize = 24_000 * 2;

fn reply_has_begun(asst_text: &str, queue_len: usize, held: bool) -> bool {
    !asst_text.is_empty() || queue_len > 0 || held
}

fn play_response(transcript: &Transcript, muted: &HashSet<String>, wire: &str) -> bool {
    if wire.is_empty() || muted.contains(wire) {
        return false;
    }
    match transcript.by_wire(wire).and_then(|id| transcript.by_id(id)) {
        Some(i) if i.kind == ItemKind::Assistant && i.status.in_flight() => true,
        _ => false,
    }
}

fn item_id(ev: &Value) -> Option<&str> {
    ev.get("item_id")
        .and_then(|v| v.as_str())
        .or_else(|| ev.pointer("/item/id").and_then(|v| v.as_str()))
}

fn response_id(ev: &Value) -> Option<&str> {
    ev.pointer("/response/id")
        .and_then(|v| v.as_str())
        .or_else(|| ev.get("response_id").and_then(|v| v.as_str()))
}

fn transcript_text(ev: &Value) -> Option<String> {
    ev.get("transcript")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            ev.pointer("/item/content/0/transcript")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
}

fn error_summary(ev: &Value) -> String {
    let err = ev.get("error").unwrap_or(ev);
    let code = err
        .get("code")
        .or_else(|| err.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("error");
    let msg = err.get("message").and_then(|v| v.as_str()).unwrap_or("");
    if msg.is_empty() { code.to_string() } else { format!("{code}: {msg}") }
}

#[cfg(test)]
mod tests {
    use super::{play_response, reply_has_begun};
    use lb_rs::Uuid;
    use lb_rs::model::chat::{Content, Event, EventBody, ItemKind, ItemMeta, Status, Transcript};
    use std::collections::HashSet;

    fn open(item: Uuid, kind: ItemKind) -> Event {
        Event::new("t", 1, EventBody::Open { item, parent: None, kind })
    }

    fn wire(item: Uuid, id: &str) -> Event {
        Event::new(
            "t",
            2,
            EventBody::SetMeta {
                item,
                meta: ItemMeta { wire_id: Some(id.into()), ..ItemMeta::default() },
            },
        )
    }

    fn running(item: Uuid) -> Event {
        Event::new("t", 3, EventBody::SetStatus { item, status: Status::Running })
    }

    fn text(item: Uuid, s: &str) -> Event {
        Event::new(
            "t",
            4,
            EventBody::Replace { item, blocks: vec![Content::Text { text: s.into() }] },
        )
    }

    fn done(item: Uuid) -> Event {
        Event::new("t", 5, EventBody::SetStatus { item, status: Status::Done })
    }

    fn cancelled(item: Uuid) -> Event {
        Event::new("t", 5, EventBody::SetStatus { item, status: Status::Cancelled })
    }

    #[test]
    fn idle_assistant_has_not_begun() {
        assert!(!reply_has_begun("", 0, false));
    }

    #[test]
    fn text_or_queued_audio_has_begun() {
        assert!(reply_has_begun("hi", 0, false));
        assert!(reply_has_begun("", 1, false));
        assert!(reply_has_begun("", 0, true));
    }

    #[test]
    fn play_in_flight_assistant_by_wire() {
        let item = Uuid::from_u128(1);
        let t =
            Transcript::fold(&[open(item, ItemKind::Assistant), wire(item, "r1"), running(item)]);
        let muted = HashSet::new();
        assert!(play_response(&t, &muted, "r1"));
        assert!(!play_response(&t, &muted, "other"));
    }

    #[test]
    fn talking_is_vad_floor_not_waiting_asr() {
        let user = Uuid::from_u128(1);
        let speaking = Transcript::fold(&[
            Event::new("t", 1, EventBody::Open { item: user, parent: None, kind: ItemKind::User }),
            Event::new(
                "t",
                2,
                EventBody::Speech { item: user, phase: lb_rs::model::chat::SpeechPhase::Start },
            ),
        ]);
        assert!(speaking.user_floor.is_some());

        let waiting_asr = Transcript::fold(&[
            Event::new("t", 1, EventBody::Open { item: user, parent: None, kind: ItemKind::User }),
            Event::new(
                "t",
                2,
                EventBody::Speech { item: user, phase: lb_rs::model::chat::SpeechPhase::Start },
            ),
            Event::new(
                "t",
                3,
                EventBody::Speech { item: user, phase: lb_rs::model::chat::SpeechPhase::Stop },
            ),
        ]);
        assert!(waiting_asr.user_floor.is_none());
        assert_eq!(waiting_asr.items[0].status, Status::Open);
    }

    #[test]
    fn play_ignores_user_floor() {
        let user = Uuid::from_u128(1);
        let asst = Uuid::from_u128(2);
        let t = Transcript::fold(&[
            Event::new("t", 1, EventBody::Open { item: user, parent: None, kind: ItemKind::User }),
            Event::new(
                "t",
                2,
                EventBody::Speech { item: user, phase: lb_rs::model::chat::SpeechPhase::Start },
            ),
            Event::new(
                "t",
                3,
                EventBody::Open { item: asst, parent: Some(user), kind: ItemKind::Assistant },
            ),
            wire(asst, "r1"),
            running(asst),
            text(asst, "hello"),
        ]);
        assert!(t.user_floor.is_some());
        assert!(play_response(&t, &HashSet::new(), "r1"));
    }

    #[test]
    fn play_skips_muted_and_cancelled() {
        let item = Uuid::from_u128(1);
        let live =
            Transcript::fold(&[open(item, ItemKind::Assistant), wire(item, "r1"), running(item)]);
        let mut muted = HashSet::new();
        muted.insert("r1".into());
        assert!(!play_response(&live, &muted, "r1"));

        let done_t = Transcript::fold(&[
            open(item, ItemKind::Assistant),
            wire(item, "r1"),
            running(item),
            cancelled(item),
        ]);
        assert!(!play_response(&done_t, &HashSet::new(), "r1"));

        let finished = Transcript::fold(&[
            open(item, ItemKind::Assistant),
            wire(item, "r1"),
            running(item),
            text(item, "hi"),
            done(item),
        ]);
        assert!(!play_response(&finished, &HashSet::new(), "r1"));
    }
}
