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
        staging: Staging::default(),
        user_id: None,
        asr_draft: String::new(),
        asst_text: String::new(),
        last_heard: None,
        pending_create: None,
        seen_calls: HashSet::new(),
        hung_up: false,
        tool_tx,
        muted_wires: HashSet::new(),
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
                call.barge_if_needed(&mut ws, audio.as_ref()).await?;
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

#[derive(Default)]
struct StagedReply {
    wire: String,
    audio: Vec<u8>,
    text: String,
    done: bool,
}

#[derive(Default)]
struct Staging {
    active: bool,
    catchup: bool,
    replies: Vec<StagedReply>,
}

impl Staging {
    fn start(&mut self) {
        self.active = true;
        self.catchup = false;
        self.replies.clear();
    }

    fn on_created(&mut self, wire: String) -> bool {
        if self.active || self.catchup {
            self.replies.push(StagedReply {
                wire,
                audio: Vec::new(),
                text: String::new(),
                done: false,
            });
            true
        } else {
            false
        }
    }

    fn by_wire_mut(&mut self, wire: &str) -> Option<&mut StagedReply> {
        self.replies.iter_mut().rev().find(|r| r.wire == wire)
    }

    fn on_stop(&mut self) -> Option<StagedReply> {
        self.catchup = self.replies.is_empty();
        if self.catchup {
            self.active = false;
            return None;
        }
        self.active = false;
        let last = self.replies.pop();
        self.replies.clear();
        last
    }

    fn take_last(&mut self) -> Option<StagedReply> {
        self.catchup = false;
        self.active = false;
        let last = self.replies.pop();
        self.replies.clear();
        last
    }

    fn abort(&mut self) {
        self.active = false;
        self.catchup = false;
        self.replies.clear();
    }
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
    staging: Staging,
    user_id: Option<Uuid>,
    asr_draft: String,
    asst_text: String,
    last_heard: Option<Uuid>,
    pending_create: Option<Uuid>,
    seen_calls: HashSet<String>,
    hung_up: bool,
    tool_tx: Sender<ToolDone>,
    /// Response ids whose audio/text we refused to play (log once).
    muted_wires: HashSet<String>,
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
            staging = self.staging.active,
            catchup = self.staging.catchup,
            staged = self.staging.replies.len(),
            pending = self.pending_create.is_some(),
            on_turn = self.transcript.on_turn_assistant().is_some(),
            asr = self.asr_draft.len(),
            should_create = self.transcript.should_create(),
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
            staging = self.staging.active,
            catchup = self.staging.catchup,
            staged = self.staging.replies.len(),
            pending = self.pending_create.is_some(),
            think = self.n_thinking(),
            on_turn = self.transcript.on_turn_assistant().is_some(),
            asr = self.asr_draft.len(),
            "chat call ws"
        );
    }

    async fn maybe_create(&mut self, ws: &mut super::realtime::VoiceConn) -> Result<(), String> {
        if self.staging.active || self.staging.catchup {
            return Ok(());
        }
        if !self.transcript.should_create() || self.transcript.items.is_empty() {
            return Ok(());
        }
        let item = Uuid::new_v4();
        let parent = self.transcript.items.last().map(|i| i.id);
        self.emit(EventBody::Open { item, parent, kind: ItemKind::Assistant });
        self.emit(EventBody::SetStatus { item, status: Status::Running });
        self.pending_create = Some(item);
        self.note_thinking();
        info!(
            %item,
            floor = self.transcript.user_floor.is_some(),
            think = self.n_thinking(),
            "chat call response.create"
        );
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
        self.staging.abort();
        if self.transcript.on_turn_assistant().is_some() {
            self.barge_if_needed(ws, audio).await?;
        }
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
        self.maybe_create(ws).await
    }

    async fn user_began(
        &mut self, ws: &mut super::realtime::VoiceConn, audio: &super::audio::Duplex,
    ) -> Result<(), String> {
        // Server VAD restarts after a short pause. If the assistant has not
        // produced audio or text yet, that restart is a continuation of the
        // same turn — cancelling it is what left unanswered questions until
        // a follow-up "Hello?". A Running client tool is a real wait; speech
        // then is a barge, not a blip.
        if self.transcript.on_turn_assistant().is_some()
            && !reply_has_begun(&self.asst_text, audio.queue_len())
            && !self.tools_running()
        {
            let elapsed_ms = self
                .think_at
                .map(|t| t.elapsed().as_millis() as u64)
                .unwrap_or(0);
            if self.user_id.is_none() {
                warn!(
                    think = self.n_thinking(),
                    elapsed_ms,
                    queue = audio.queue_len(),
                    "chat call speech hidden: pending reply"
                );
            } else {
                info!(
                    user = %self.user_id.unwrap(),
                    think = self.n_thinking(),
                    elapsed_ms,
                    "chat call vad continue pending reply"
                );
            }
            return Ok(());
        }
        self.barge_if_needed(ws, Some(audio)).await?;
        if let Some(id) = self.user_id {
            info!(%id, "chat call vad already on user");
            return Ok(());
        }
        self.staging.start();
        if let Some(id) = self.resume_user() {
            info!(%id, "chat call vad resume empty user");
            self.emit(EventBody::Speech { item: id, phase: SpeechPhase::Start });
            self.emit(EventBody::SetStatus { item: id, status: Status::Open });
            self.user_id = Some(id);
            return Ok(());
        }
        let item = Uuid::new_v4();
        let parent = self.transcript.items.last().map(|i| i.id);
        self.emit(EventBody::Open { item, parent, kind: ItemKind::User });
        self.emit(EventBody::Speech { item, phase: SpeechPhase::Start });
        self.user_id = Some(item);
        self.asr_draft.clear();
        info!(%item, "chat call vad start");
        Ok(())
    }

    fn resume_user(&self) -> Option<Uuid> {
        let id = self.last_heard?;
        let item = self.transcript.by_id(id)?;
        if item.kind == ItemKind::User && !item.has_text() { Some(id) } else { None }
    }

    fn close_user_item(&mut self) {
        let Some(item) = self.user_id.take() else {
            return;
        };
        self.emit(EventBody::Speech { item, phase: SpeechPhase::Stop });
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

    fn promote_staged(&mut self, reply: StagedReply, audio: Option<&super::audio::Duplex>) {
        self.staging.active = false;
        self.staging.catchup = false;
        let item = Uuid::new_v4();
        let parent = self.transcript.items.last().map(|i| i.id);
        self.emit(EventBody::Open { item, parent, kind: ItemKind::Assistant });
        self.emit(EventBody::SetMeta {
            item,
            meta: ItemMeta { wire_id: Some(reply.wire.clone()), ..ItemMeta::default() },
        });
        if !reply.text.is_empty() {
            self.emit(EventBody::Replace {
                item,
                blocks: vec![Content::Text { text: reply.text.clone() }],
            });
        }
        if let Some(audio) = audio {
            audio.barge_in();
            if !reply.audio.is_empty() {
                audio.push_voice_pcm16(&reply.audio);
            }
        }
        let chars = reply.text.len();
        let audio_n = reply.audio.len();
        if reply.done {
            self.emit(EventBody::SetStatus { item, status: Status::Done });
            self.asst_text.clear();
        } else {
            self.emit(EventBody::SetStatus { item, status: Status::Running });
            self.asst_text = reply.text;
        }
        self.note_thinking();
        info!(
            %item,
            wire = %reply.wire,
            chars,
            audio = audio_n,
            done = reply.done,
            "chat call promote staged"
        );
    }

    fn catch_up(&mut self, audio: Option<&super::audio::Duplex>) {
        if self.asr_draft.is_empty() {
            // VAD often ends before ASR. Keep the user row open and accept the
            // server-started response instead of aborting the turn.
            let staged = self.staging.replies.len();
            let reply = self.staging.on_stop();
            if let Some(reply) = reply {
                warn!(
                    wire = %reply.wire,
                    chars = reply.text.len(),
                    audio = reply.audio.len(),
                    done = reply.done,
                    "chat call drop staged: empty asr"
                );
            } else {
                info!(
                    catchup = self.staging.catchup,
                    staged,
                    user = ?self.user_id,
                    "chat call speech_stopped empty asr"
                );
            }
            return;
        }
        self.close_user_item();
        if let Some(reply) = self.staging.on_stop() {
            self.promote_staged(reply, audio);
        }
    }

    fn apply_asr(&mut self, item: Uuid, text: String) {
        if text.is_empty() {
            return;
        }
        let prev = self
            .transcript
            .by_id(item)
            .map(|i| i.text())
            .unwrap_or_default();
        if prev == text {
            return;
        }
        if self.user_id == Some(item) {
            self.asr_draft = text.clone();
        } else {
            info!(
                target = %item,
                user = ?self.user_id,
                last = ?self.last_heard,
                chars = text.len(),
                "chat call asr on other item"
            );
        }
        self.emit(EventBody::Replace { item, blocks: vec![Content::Text { text }] });
        if self.user_id == Some(item) && !self.staging.active {
            self.close_user_item();
        }
    }

    async fn barge_if_needed(
        &mut self, ws: &mut super::realtime::VoiceConn, audio: Option<&super::audio::Duplex>,
    ) -> Result<(), String> {
        let on_turn = self.transcript.on_turn_assistant();
        if let Some(audio) = audio {
            if audio.queue_len() > 0 {
                audio.barge_in();
            }
        }
        let Some(item) = on_turn else {
            return Ok(());
        };
        if self.pending_create == Some(item) {
            self.pending_create = None;
        }
        let _ = ws.response_cancel().await;
        info!(
            %item,
            chars = self.asst_text.len(),
            pending = self.pending_create.is_some(),
            "chat call barge"
        );
        if !self.asst_text.is_empty() {
            self.emit(EventBody::Replace {
                item,
                blocks: vec![Content::Text { text: self.asst_text.clone() }],
            });
        }
        self.emit(EventBody::SetStatus { item, status: Status::Cancelled });
        self.asst_text.clear();
        self.clear_thinking_if_idle();
        Ok(())
    }

    fn hangup(&mut self, _queue: usize) {
        if self.hung_up {
            return;
        }
        self.hung_up = true;
        if self.user_id.is_some() {
            self.close_user_item();
        }
        if let Some(item) = self.transcript.on_turn_assistant() {
            if !self.asst_text.is_empty() {
                self.emit(EventBody::Replace {
                    item,
                    blocks: vec![Content::Text { text: self.asst_text.clone() }],
                });
                self.asst_text.clear();
            }
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
                if let Some(duplex) = audio.as_deref() {
                    self.user_began(ws, duplex).await?;
                }
                if let (Some(id), Some(wire)) = (self.user_id, item_id(ev)) {
                    self.emit(EventBody::SetMeta {
                        item: id,
                        meta: ItemMeta { wire_id: Some(wire.to_string()), ..ItemMeta::default() },
                    });
                }
            }
            "input_audio_buffer.speech_stopped" => {
                self.catch_up(audio.as_deref());
            }
            "response.created" => {
                let wire = response_id(ev).unwrap_or("").to_string();
                if self.staging.on_created(wire.clone()) {
                    info!(wire, catchup = self.staging.catchup, "chat call response staged");
                    if self.staging.catchup {
                        if let Some(reply) = self.staging.take_last() {
                            self.promote_staged(reply, audio.as_deref());
                        }
                    }
                } else if let Some(item) = self.pending_create.take() {
                    info!(%item, wire, "chat call response bind pending");
                    self.emit(EventBody::SetMeta {
                        item,
                        meta: ItemMeta { wire_id: Some(wire), ..ItemMeta::default() },
                    });
                } else if let Some(item) = self.transcript.on_turn_assistant().filter(|id| {
                    self.transcript
                        .by_id(*id)
                        .and_then(|i| i.meta.wire_id.as_deref())
                        .is_none()
                }) {
                    info!(%item, wire, "chat call response bind on_turn");
                    self.emit(EventBody::SetMeta {
                        item,
                        meta: ItemMeta { wire_id: Some(wire), ..ItemMeta::default() },
                    });
                } else {
                    let item = Uuid::new_v4();
                    let parent = self.transcript.items.last().map(|i| i.id);
                    warn!(
                        %item,
                        wire,
                        think = self.n_thinking(),
                        on_turn = self.transcript.on_turn_assistant().is_some(),
                        "chat call extra thinking"
                    );
                    self.emit(EventBody::Open { item, parent, kind: ItemKind::Assistant });
                    self.emit(EventBody::SetMeta {
                        item,
                        meta: ItemMeta { wire_id: Some(wire), ..ItemMeta::default() },
                    });
                    self.emit(EventBody::SetStatus { item, status: Status::Running });
                    self.asst_text.clear();
                    self.note_thinking();
                }
            }
            "response.output_audio.delta" | "response.audio.delta" => {
                if let Some(wire) = response_id(ev) {
                    if let Some(staged) = self.staging.by_wire_mut(wire) {
                        if let Some(b64) = ev.get("delta").and_then(|v| v.as_str()) {
                            if let Ok(bytes) = decode_b64(b64) {
                                staged.audio.extend_from_slice(&bytes);
                            }
                        }
                        return Ok(());
                    }
                }
                if self.should_play(ev) {
                    if let (Some(duplex), Some(b64)) =
                        (audio, ev.get("delta").and_then(|v| v.as_str()))
                    {
                        if let Ok(bytes) = decode_b64(b64) {
                            duplex.push_voice_pcm16(&bytes);
                        }
                    }
                }
            }
            "response.output_audio_transcript.delta" | "response.output_text.delta" => {
                if let Some(wire) = response_id(ev) {
                    if let Some(staged) = self.staging.by_wire_mut(wire) {
                        if let Some(d) = ev.get("delta").and_then(|v| v.as_str()) {
                            staged.text.push_str(d);
                        }
                        return Ok(());
                    }
                }
                if self.should_play(ev) {
                    if let Some(d) = ev.get("delta").and_then(|v| v.as_str()) {
                        self.asst_text.push_str(d);
                        if let Some(item) = self.transcript.on_turn_assistant() {
                            self.emit(EventBody::Replace {
                                item,
                                blocks: vec![Content::Text { text: self.asst_text.clone() }],
                            });
                            self.clear_thinking_if_idle();
                        }
                    }
                }
            }
            "response.done" => {
                let Some(wire) = response_id(ev) else {
                    return Ok(());
                };
                if let Some(staged) = self.staging.by_wire_mut(wire) {
                    staged.done = true;
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
                if !self.asst_text.is_empty() {
                    self.emit(EventBody::Replace {
                        item,
                        blocks: vec![Content::Text { text: self.asst_text.clone() }],
                    });
                    self.asst_text.clear();
                }
                self.emit(EventBody::SetStatus { item, status: Status::Done });
                self.clear_thinking_if_idle();
            }
            "response.function_call_arguments.done" => {
                if self.staging.active || self.staging.catchup {
                    warn!(
                        staging = self.staging.active,
                        catchup = self.staging.catchup,
                        "chat call drop function_call: staging"
                    );
                    return Ok(());
                }
                self.handle_tool(ev, ws, audio).await?;
            }
            "conversation.item.input_audio_transcription.updated"
            | "conversation.item.input_audio_transcription.completed" => {
                if let Some(text) = transcript_text(ev) {
                    let id = item_id(ev)
                        .and_then(|w| self.transcript.by_wire(w))
                        .or(self.user_id)
                        .or(self.last_heard);
                    if let Some(id) = id {
                        self.apply_asr(id, text);
                    } else if ev_type(ev).ends_with("completed") {
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
        let parent = self
            .transcript
            .on_turn_assistant()
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

    fn should_play(&mut self, ev: &Value) -> bool {
        let wire = response_id(ev).unwrap_or("");
        if self.user_id.is_some() || self.transcript.user_floor.is_some() {
            if !wire.is_empty() && self.muted_wires.insert(wire.to_string()) {
                warn!(
                    wire,
                    user = ?self.user_id,
                    floor = self.transcript.user_floor.is_some(),
                    "chat call mute response"
                );
            }
            return false;
        }
        if wire.is_empty() {
            return false;
        }
        let play = self
            .transcript
            .on_turn_assistant()
            .and_then(|id| self.transcript.by_id(id))
            .and_then(|item| item.meta.wire_id.as_deref())
            == Some(wire);
        if !play && self.muted_wires.insert(wire.to_string()) {
            warn!(
                wire,
                on_turn = ?self.transcript.on_turn_assistant(),
                "chat call mute unmatched wire"
            );
        }
        play
    }
}

fn reply_has_begun(asst_text: &str, queue_len: usize) -> bool {
    !asst_text.is_empty() || queue_len > 0
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
    use super::reply_has_begun;

    #[test]
    fn idle_assistant_has_not_begun() {
        assert!(!reply_has_begun("", 0));
    }

    #[test]
    fn text_or_queued_audio_has_begun() {
        assert!(reply_has_begun("hi", 0));
        assert!(reply_has_begun("", 1));
    }
}
