//! The turn loop. One driver per chat, on its own thread. A command starts a
//! run: completions with tool calls until the model stops calling tools,
//! each finished piece settled into the store as it lands. Stop cancels the
//! stream and settles what was streamed. The thread is synchronous; only the
//! streaming completion runs on the runtime, so the store and the tools may
//! block freely. A voice session is the exception: it holds the runtime
//! until it ends and steps out of it to write lines and run tools.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use lb_rs::Uuid;
use lb_rs::model::chat::{Body, Chat, Entry, Mention, Settings};
use serde_json::json;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::error::TryRecvError;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tracing::warn;

use crate::context::{self, truncate};
use crate::provider::Provider;
use crate::store::Store;
use crate::territory::Territory;
use crate::tools::{ToolOutcome, Tools};
use crate::voice;
use crate::web;
use crate::wire::{self, Call, Piece, Request, images};

/// Bytes of a tool result written to the chat.
pub const TOOL_RESULT_CAP: usize = 16 * 1024;
/// The same for the device's own model, whose window is small.
const FENCED_RESULT_CAP: usize = 8 * 1024;
/// Added to the prompt while the device is offline.
const OFFLINE: &str = " The device is offline right now: web search and fetch will not work \
    until it is back, so say so when asked for something they would be needed for.";

/// What a call is answered with once it has answered the same twice.
const REPEATED: &str = "you have made this call twice already and it answered the same; \
    do something else or answer the user";

pub enum Cmd {
    Say {
        text: String,
        mentions: Vec<Mention>,
    },
    /// Replace the message `id` and everything after it, then run.
    Edit {
        id: Uuid,
        text: String,
        mentions: Vec<Mention>,
    },
    /// Drop everything after the last message and run again.
    Regenerate,
    /// Run on from where the last turn stopped, keeping what it had done.
    Resume,
    /// Persist this user's settings without running.
    SetSettings(Settings),
    /// Open a spoken conversation on the chat; `Stop` ends it.
    StartVoice,
    /// The user's voice, as PCM16 mono at `realtime::RATE`.
    Audio(Vec<u8>),
    /// How much of a reply has been played so far.
    Played {
        reply: u32,
        ms: u64,
    },
    Stop,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    RunStarted,
    Delta(String),
    /// More of what the model shows of its thinking.
    Thinking(String),
    ToolStarted(Call),
    /// A line settled into the chat.
    Written(Entry),
    /// A line could not be written to the chat; the run stops.
    Lost {
        entry: Entry,
        error: String,
    },
    RunEnded,
    /// A spoken conversation is open: the microphone should run.
    VoiceStarted,
    VoiceEnded,
    /// More of reply `reply`, as PCM16 mono at `realtime::RATE`: play it,
    /// and report what has played with `Cmd::Played`.
    Audio {
        reply: u32,
        pcm: Vec<u8>,
    },
    /// The user spoke over the reply: drop what is queued to play.
    Interrupted,
    /// More of what the user is saying, as it is made out; their line
    /// settles when they are done.
    Hearing(String),
}

pub struct Config {
    pub user: String,
    pub working_dir: String,
    pub provider: Box<dyn Fn() -> Result<Provider, String> + Send>,
    /// The model's context window in tokens, when known. Asked before every
    /// completion, so it remembers what it learned.
    pub window: fn(&Provider) -> Option<u64>,
}

pub struct Driver {
    cmds: UnboundedSender<Cmd>,
    events: Receiver<Event>,
    busy: Arc<AtomicBool>,
}

/// A way to send the driver commands from any thread: a host's audio
/// engine feeds a voice session through one.
#[derive(Clone)]
pub struct Handle(UnboundedSender<Cmd>);

impl Handle {
    pub fn send(&self, cmd: Cmd) {
        let _ = self.0.send(cmd);
    }

    /// Whether `other` leads to the same driver.
    pub fn is(&self, other: &Handle) -> bool {
        self.0.same_channel(&other.0)
    }

    /// A handle onto a channel of one's own, for a host's tests.
    pub fn from_sender(sender: UnboundedSender<Cmd>) -> Handle {
        Handle(sender)
    }
}

impl Driver {
    /// `wake` is called after every event, from the driver thread.
    pub fn spawn(
        store: impl Store + 'static, tools: impl Tools + 'static, config: Config,
        wake: impl Fn() + Send + Sync + 'static,
    ) -> Driver {
        let (cmds, cmd_rx) = unbounded_channel();
        let (event_tx, events) = channel();
        let busy = Arc::new(AtomicBool::new(false));
        let worker_busy = busy.clone();
        std::thread::Builder::new()
            .name("lb-chat".into())
            .spawn(move || {
                // Multi-threaded so a voice session can step out of it.
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .expect("runtime");
                let client = rt.block_on(async { reqwest::Client::new() });
                Worker {
                    lines: Lines { store: Box::new(store), events: event_tx, wake: Box::new(wake) },
                    tools: Box::new(tools),
                    config,
                    busy: worker_busy,
                    client,
                    rt,
                    made: Made::default(),
                    artist: None,
                    result_cap: TOOL_RESULT_CAP,
                }
                .run(cmd_rx);
            })
            .expect("spawn chat driver");
        Driver { cmds, events, busy }
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.cmds.send(cmd);
    }

    pub fn handle(&self) -> Handle {
        Handle(self.cmds.clone())
    }

    pub fn poll(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }

    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::Relaxed)
    }
}

struct Worker {
    lines: Lines,
    tools: Box<dyn Tools>,
    config: Config,
    busy: Arc<AtomicBool>,
    client: reqwest::Client,
    rt: Runtime,
    made: Made,
    /// The provider and model that make pictures for this round's model.
    artist: Option<(Provider, &'static str)>,
    /// Bytes of a tool result kept, for this round's model.
    result_cap: usize,
}

/// Where settled lines go, and who is told of them.
pub(crate) struct Lines {
    pub(crate) store: Box<dyn Store>,
    events: Sender<Event>,
    wake: Box<dyn Fn() + Send + Sync>,
}

impl Lines {
    /// Appends `entry`; false means it was lost and the run should stop.
    pub(crate) fn settle(&self, entry: Entry) -> bool {
        match self.store.append(entry.clone()) {
            Ok(stored) => {
                self.emit(Event::Written(stored));
                true
            }
            Err(error) => {
                self.emit(Event::Lost { entry, error });
                false
            }
        }
    }

    pub(crate) fn emit(&self, event: Event) {
        let _ = self.events.send(event);
        (self.wake)();
    }
}

/// The calls made in a run and what each answered: one made again to the
/// same answer is refused, and made once more it ends the run.
#[derive(Default)]
pub(crate) struct Made(Vec<MadeCall>);

struct MadeCall {
    name: String,
    args: serde_json::Value,
    answer: Option<(String, bool)>,
    repeats: usize,
}

impl Made {
    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }

    /// Runs `call` through `run`, unless it has answered the same twice.
    pub(crate) fn call(
        &mut self, call: &Call, run: impl FnOnce(&Call) -> ToolOutcome,
    ) -> ToolOutcome {
        let same = |m: &MadeCall| m.name == call.name && m.args == call.args;
        let at = self.0.iter().position(same).unwrap_or_else(|| {
            let (name, args) = (call.name.clone(), call.args.clone());
            self.0
                .push(MadeCall { name, args, answer: None, repeats: 0 });
            self.0.len() - 1
        });
        match self.0[at].repeats {
            0 => {}
            1 => {
                self.0[at].repeats = 2;
                return ToolOutcome::err(REPEATED);
            }
            _ => return ToolOutcome::Abort { text: format!("{} kept repeating", call.name) },
        }
        let outcome = run(call);
        if let ToolOutcome::Done { text, ok } = &outcome {
            let answer = Some((text.clone(), *ok));
            if self.0[at].answer == answer {
                self.0[at].repeats = 1;
            } else {
                // A new answer is progress: earlier repeats no longer count.
                self.0.iter_mut().for_each(|m| m.repeats = 0);
                self.0[at].answer = answer;
            }
        }
        outcome
    }
}

enum Outcome {
    Finished(wire::Completion),
    Stopped,
    Failed(String),
}

impl Worker {
    fn run(mut self, mut cmds: UnboundedReceiver<Cmd>) {
        while let Some(cmd) = cmds.blocking_recv() {
            let user = self.config.user.clone();
            let staged = match cmd {
                Cmd::Say { text, mentions } => self
                    .lines
                    .store
                    .append(said(&user, text, mentions))
                    .map(Some),
                Cmd::Edit { id, text, mentions } => {
                    let entry = said(&user, text, mentions);
                    self.lines
                        .store
                        .update(&mut |chat| {
                            chat.truncate_from(id, &user);
                            chat.push(entry.clone());
                        })
                        .map(|chat| chat.entries.last().cloned())
                }
                Cmd::Regenerate => self
                    .lines
                    .store
                    .update(&mut |chat| {
                        chat.truncate_after_last_user(&user);
                    })
                    .map(|_| None),
                Cmd::Resume => {
                    self.turn(&mut cmds, false);
                    continue;
                }
                Cmd::SetSettings(settings) => {
                    let saved = self
                        .lines
                        .store
                        .update(&mut |chat| chat.set_settings(&user, settings.clone()));
                    if let Err(err) = saved {
                        self.lines.settle(Entry::error(&user, err));
                    }
                    continue;
                }
                Cmd::StartVoice => {
                    self.voice(&mut cmds);
                    continue;
                }
                Cmd::Audio(_) | Cmd::Played { .. } | Cmd::Stop => continue,
            };
            match staged {
                Ok(entry) => {
                    if let Some(entry) = entry {
                        self.lines.emit(Event::Written(entry));
                    }
                    self.turn(&mut cmds, true);
                }
                Err(err) => {
                    self.lines.settle(Entry::error(&user, err));
                }
            }
        }
    }

    /// A run: what the newest message attached is read first, unless the
    /// run goes on from a stop, when that was done.
    fn turn(&mut self, cmds: &mut UnboundedReceiver<Cmd>, attached: bool) {
        self.busy.store(true, Ordering::Relaxed);
        self.lines.emit(Event::RunStarted);
        self.made.clear();
        // Attached notes are read before the first completion prepares.
        if let Ok(provider) = (self.config.provider)() {
            self.result_cap = cap_for(&provider);
        }
        if !attached || self.read_attached(cmds) {
            self.rounds(cmds);
        }
        self.busy.store(false, Ordering::Relaxed);
        self.lines.emit(Event::RunEnded);
    }

    /// Holds a spoken conversation on the chat until told to stop.
    fn voice(&mut self, cmds: &mut UnboundedReceiver<Cmd>) {
        let user = self.config.user.clone();
        let ready = self.prepare(true).and_then(|(provider, request)| {
            let model = provider
                .speaks()
                .ok_or_else(|| format!("{} has no model that speaks", provider.label()))?;
            Ok((provider, model, request))
        });
        let (provider, model, request) = match ready {
            Ok(ready) => ready,
            Err(err) => {
                self.lines.settle(Entry::error(&user, err));
                self.lines.emit(Event::VoiceEnded);
                return;
            }
        };
        self.made.clear();
        let Worker { lines, tools, config, busy, rt, made, .. } = self;
        let host = voice::Host {
            user: &config.user,
            model: format!("{}/{model}", provider.name),
            working_dir: &config.working_dir,
            lines,
            tools: &mut **tools,
            made,
            busy,
        };
        rt.block_on(voice::run(host, &provider, &model, request, cmds));
    }

    /// Reads what the newest message attached, as tool lines: the model
    /// keeps each note as it was then. Returns whether the run continues.
    fn read_attached(&mut self, cmds: &mut UnboundedReceiver<Cmd>) -> bool {
        let user = self.config.user.clone();
        let Ok((_, bytes)) = self.lines.store.load() else { return true };
        let chat = Chat::parse(&bytes);
        let newest = chat.entries.iter().rfind(|e| e.from == user);
        let Some(Body::User { mentions, .. }) = newest.map(|e| &e.body) else { return true };
        self.tools.prepare(&chat, &user, &self.config.working_dir);
        let calls = mentions
            .iter()
            .map(|mention| Call {
                id: Uuid::new_v4().to_string(),
                name: "read".into(),
                args: json!({ "path": self.tools.locate(mention) }),
                echo: None,
            })
            .collect();
        self.run_tools(calls, cmds)
    }

    fn rounds(&mut self, cmds: &mut UnboundedReceiver<Cmd>) {
        let user = self.config.user.clone();
        loop {
            let (provider, request) = match self.prepare(false) {
                Ok(ready) => ready,
                Err(err) => {
                    self.lines.settle(Entry::error(&user, err));
                    break;
                }
            };
            let (heard, outcome) = self.complete(&provider, &request, cmds);
            match outcome {
                Outcome::Stopped => {
                    if !heard.text.trim().is_empty() {
                        self.lines.settle(reply(&user, &provider, &heard, true));
                    }
                    break;
                }
                Outcome::Failed(err) => {
                    self.lines.settle(Entry::error(&user, err));
                    break;
                }
                Outcome::Finished(mut completion) => {
                    keep_made(&mut *self.tools, &mut completion);
                    // What the provider ran itself came before what it said.
                    let served = completion.served.iter().map(|s| {
                        let result = if s.result.is_empty() { "done" } else { &s.result };
                        let mut entry = Entry::tool(&user, &s.name, s.args.clone(), result, true);
                        if let Body::Tool { server, .. } = &mut entry.body {
                            *server = true;
                        }
                        if let Some(echo) = &s.echo {
                            entry.extra.insert("echo".into(), json!(echo));
                        }
                        entry
                    });
                    if !served
                        .collect::<Vec<_>>()
                        .into_iter()
                        .all(|e| self.lines.settle(e))
                    {
                        break;
                    }
                    let said = reply(&user, &provider, &completion, false);
                    if !self.lines.settle(said) || completion.calls.is_empty() {
                        break;
                    }
                    if !self.run_tools(completion.calls, cmds) {
                        break;
                    }
                }
            }
        }
    }

    /// Runs the calls in order. Returns whether the run continues.
    fn run_tools(&mut self, calls: Vec<Call>, cmds: &mut UnboundedReceiver<Cmd>) -> bool {
        let user = self.config.user.clone();
        for call in calls {
            self.lines.emit(Event::ToolStarted(call.clone()));
            let (text, ok) = match self.call(&call) {
                ToolOutcome::Done { text, ok } => (text, ok),
                ToolOutcome::Abort { text } => {
                    self.lines
                        .settle(Entry::error(&user, format!("stopped: {text}")));
                    return false;
                }
            };
            let result = truncate(&text, self.result_cap);
            let mut entry = Entry::tool(&user, call.name, call.args, result, ok);
            if let Some(echo) = call.echo.and_then(|e| serde_json::to_value(e).ok()) {
                entry.extra.insert("echo".into(), echo);
            }
            if !self.lines.settle(entry) {
                return false;
            }
            // Told to stop, or nothing holds the driver any more.
            if matches!(cmds.try_recv(), Ok(Cmd::Stop) | Err(TryRecvError::Disconnected)) {
                return false;
            }
        }
        true
    }

    /// Runs `call`, the artist drawing when it asks for a picture.
    fn call(&mut self, call: &Call) -> ToolOutcome {
        let Worker { made, tools, artist, client, rt, .. } = self;
        let artist = artist.as_ref().filter(|_| call.name == images::NAME);
        made.call(call, |call| match artist {
            Some((provider, model)) => draw(client, rt, &mut **tools, provider, model, call),
            None => tools.call(call),
        })
    }

    /// The provider and request for the next completion; `spoken` keeps
    /// every tool of ours, there being nothing the voice server runs itself.
    fn prepare(&mut self, spoken: bool) -> Result<(Provider, Request), String> {
        let (_, bytes) = self.lines.store.load()?;
        let chat = Chat::parse(&bytes);
        let provider = (self.config.provider)()?;
        if provider.needs_key {
            return Err(format!("{} needs an API key", provider.label()));
        }
        let settings = chat.settings_for(&self.config.user);
        let territory = Territory::new(&self.config.working_dir, &settings);
        self.tools
            .prepare(&chat, &self.config.user, &self.config.working_dir);
        let fenced = provider.fenced();
        let instructions =
            if fenced { Vec::new() } else { self.tools.instructions(&self.config.working_dir) };
        let mut system = context::system_prompt(&territory, &instructions, fenced);
        if self.tools.offline() {
            system.push_str(OFFLINE);
        }
        let mut tools = self.tools.schemas();
        if provider.reaches_the_web() && !spoken {
            tools.retain(|tool| tool.name != web::SEARCH && tool.name != web::FETCH);
        }
        self.result_cap = cap_for(&provider);
        if fenced {
            tools.retain(|tool| {
                ["search", "read", "list", web::SEARCH, web::FETCH].contains(&tool.name.as_str())
            });
            tools.iter_mut().for_each(slim);
        }
        self.artist = provider
            .draws()
            .filter(|_| !spoken)
            .map(|model| (provider.clone(), model));
        if self.artist.is_some() {
            tools.push(images::schema());
        }
        // Tool results get half of what the prompt and the schemas leave,
        // at four bytes a token.
        let budget = (self.config.window)(&provider).map(|window| {
            let fixed = tools.iter().fold(system.len(), |sum, tool| {
                sum + tool.name.len() + tool.description.len() + tool.parameters.to_string().len()
            });
            (window as usize * 4).saturating_sub(fixed) / 2
        });
        let (sees, reads) = (provider.sees(), provider.reads_pdfs());
        let eyes = &mut self.tools;
        let mut see = |path: &str| {
            eyes.media(path)
                .filter(|m| if m.is_pdf() { reads } else { sees })
        };
        let turns = context::turns(&chat, &self.config.user, budget, &mut see);
        let request = Request { system, turns, tools, effort: provider.effort.clone() };
        Ok((provider, request))
    }

    fn complete(
        &self, provider: &Provider, request: &Request, cmds: &mut UnboundedReceiver<Cmd>,
    ) -> (wire::Completion, Outcome) {
        let (delta_tx, mut deltas) = unbounded_channel::<Piece>();
        let mut heard = wire::Completion::default();
        let outcome = self.rt.block_on(async {
            let future = wire::complete(&self.client, provider, request, &delta_tx);
            tokio::pin!(future);
            loop {
                tokio::select! {
                    result = &mut future => break match result {
                        Ok(completion) => Outcome::Finished(completion),
                        Err(err) => Outcome::Failed(err),
                    },
                    Some(piece) = deltas.recv() => self.hear(&mut heard, piece),
                    cmd = cmds.recv() => match cmd {
                        Some(Cmd::Stop) | None => break Outcome::Stopped,
                        Some(_) => warn!("chat command ignored while a run is live"),
                    },
                }
            }
        });
        while let Ok(piece) = deltas.try_recv() {
            self.hear(&mut heard, piece);
        }
        (heard, outcome)
    }

    fn hear(&self, heard: &mut wire::Completion, piece: Piece) {
        match piece {
            Piece::Text(text) => {
                heard.text.push_str(&text);
                self.lines.emit(Event::Delta(text));
            }
            Piece::Thinking(text) => {
                heard.thinking.push_str(&text);
                self.lines.emit(Event::Thinking(text));
            }
        }
    }
}

/// Bytes of a tool result kept for `provider`'s model.
fn cap_for(provider: &Provider) -> usize {
    if provider.fenced() { FENCED_RESULT_CAP } else { TOOL_RESULT_CAP }
}

/// Leaves a tool its required arguments and the few optional ones reading
/// needs: the device's own model reaches for the rest and gets them wrong.
fn slim(tool: &mut wire::ToolSchema) {
    const KEPT: &[(&str, &str)] = &[("read", "section"), ("list", "path")];
    let required: Vec<String> = tool.parameters["required"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    if let Some(props) = tool.parameters["properties"].as_object_mut() {
        props.retain(|name, _| {
            required.contains(name) || KEPT.contains(&(tool.name.as_str(), name.as_str()))
        });
    }
}

/// Has the provider make the picture `call` asks for, and keeps it.
fn draw(
    client: &reqwest::Client, rt: &Runtime, tools: &mut dyn Tools, provider: &Provider,
    model: &str, call: &Call,
) -> ToolOutcome {
    let prompt = call.args["prompt"].as_str().unwrap_or_default();
    let kept = rt
        .block_on(images::generate(client, provider, model, prompt))
        .and_then(|(ext, bytes)| tools.keep(&picture_name(&ext), &bytes));
    match kept {
        Ok(path) => ToolOutcome::ok(path),
        Err(e) => ToolOutcome::err(e),
    }
}

/// The line for a message of the user's and what it attached.
fn said(user: &str, text: String, mentions: Vec<Mention>) -> Entry {
    let mut entry = Entry::user(user, text);
    if let Body::User { mentions: attached, .. } = &mut entry.body {
        *attached = mentions;
    }
    entry
}

/// Keeps each file the provider made beside the chat: its path becomes
/// the result of the call that made it, and it is shown under the reply.
fn keep_made(tools: &mut dyn Tools, completion: &mut wire::Completion) {
    for made in &mut completion.served {
        let Some((ext, bytes)) = made.file.take() else { continue };
        match tools.keep(&picture_name(&ext), &bytes) {
            Ok(path) => {
                completion.text.push_str(&format!("\n\n![]({path})"));
                made.result = path;
            }
            Err(e) => made.result = format!("the picture could not be kept: {e}"),
        }
    }
}

/// A name for a picture made now.
fn picture_name(ext: &str) -> String {
    format!("picture_{}.{ext}", chrono::Local::now().format("%Y-%m-%d_%H-%M-%S"))
}

/// The reply line for what a completion said, whole or cut short.
fn reply(user: &str, provider: &Provider, said: &wire::Completion, cut_short: bool) -> Entry {
    let mut entry = Entry::assistant(user, said.text.trim(), provider.selection(), said.usage);
    if let Body::Assistant { thinking, interrupted, .. } = &mut entry.body {
        *thinking = said.thinking.trim().to_string();
        *interrupted = cut_short;
    }
    entry
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;
    use crate::mock::{self, sse_call, sse_text};
    use crate::provider::Kind;
    use crate::store::MemStore;
    use crate::tools::NoTools;
    use crate::wire::ToolSchema;

    fn driver(store: MemStore, tools: impl Tools + 'static, base_url: String) -> Driver {
        let config = Config {
            user: "u".into(),
            working_dir: "/".into(),
            provider: Box::new(move || {
                Ok(Provider {
                    name: "mock".into(),
                    display_name: None,
                    needs_key: false,
                    kind: Kind::OpenAi,
                    base_url: base_url.clone(),
                    api_key: None,
                    model: "m".into(),
                    effort: None,
                })
            }),
            window: |_| None,
        };
        Driver::spawn(store, tools, config, || {})
    }

    fn wait_for(driver: &Driver, done: impl Fn(&[Event]) -> bool) -> Vec<Event> {
        let start = Instant::now();
        let mut events = Vec::new();
        while start.elapsed() < Duration::from_secs(10) {
            events.extend(driver.poll());
            if done(&events) {
                return events;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out: {events:?}");
    }

    fn wait_for_run(driver: &Driver) -> Vec<Event> {
        wait_for(driver, |events| events.contains(&Event::RunEnded))
    }

    fn kinds(chat: &Chat) -> Vec<&str> {
        chat.entries
            .iter()
            .map(|e| match &e.body {
                Body::User { .. } => "user",
                Body::Assistant { .. } => "assistant",
                Body::Tool { .. } => "tool",
                Body::Error { .. } => "error",
                _ => "other",
            })
            .collect()
    }

    /// Says it is offline.
    struct Cut;

    impl Tools for Cut {
        fn schemas(&self) -> Vec<ToolSchema> {
            Vec::new()
        }

        fn call(&mut self, _call: &Call) -> ToolOutcome {
            ToolOutcome::err("unknown")
        }

        fn offline(&self) -> bool {
            true
        }
    }

    /// The model is told when the device is offline, so a model on the
    /// device does not reach for the web.
    #[test]
    fn an_offline_device_is_said_in_the_prompt() {
        let store = MemStore::default();
        let (url, bodies) = mock::serve(vec![sse_text("ok")]);
        let d = driver(store.clone(), Cut, url);
        d.send(Cmd::Say { text: "hi".into(), mentions: vec![] });
        wait_for_run(&d);
        let sent: serde_json::Value = serde_json::from_str(&bodies.recv().unwrap()).unwrap();
        let system = sent["messages"][0]["content"].as_str().unwrap();
        assert!(
            system.ends_with(OFFLINE.trim_start()) || system.contains("offline right now"),
            "{system}"
        );
    }

    /// `echo` answers; `wall` aborts.
    struct Mock;

    impl Tools for Mock {
        fn schemas(&self) -> Vec<ToolSchema> {
            ["echo", "wall"]
                .iter()
                .map(|name| ToolSchema {
                    name: name.to_string(),
                    description: name.to_string(),
                    parameters: json!({"type": "object", "properties": {"text": {"type": "string"}}}),
                })
                .collect()
        }

        fn call(&mut self, call: &Call) -> ToolOutcome {
            match call.name.as_str() {
                "echo" => {
                    ToolOutcome::ok(format!("echo:{}", call.args["text"].as_str().unwrap_or("")))
                }
                "wall" => ToolOutcome::Abort { text: "blocked".into() },
                _ => ToolOutcome::err("unknown"),
            }
        }
    }

    /// Keeps what it is handed and says where.
    struct Shelves(Kept);
    /// Names and bytes, in the order they were kept.
    type Kept = Arc<std::sync::Mutex<Vec<(String, Vec<u8>)>>>;

    impl Tools for Shelves {
        fn schemas(&self) -> Vec<ToolSchema> {
            Vec::new()
        }

        fn call(&mut self, _call: &Call) -> ToolOutcome {
            ToolOutcome::err("unknown")
        }

        fn keep(&mut self, name: &str, bytes: &[u8]) -> Result<String, String> {
            self.0
                .lock()
                .unwrap()
                .push((name.to_string(), bytes.to_vec()));
            Ok(format!("/imports/{name}"))
        }
    }

    /// A picture the provider made is kept as a file, named on its tool
    /// line, and shown under the reply.
    #[test]
    fn a_picture_the_provider_made_is_kept_and_shown() {
        let kept = Arc::new(std::sync::Mutex::new(Vec::new()));
        let made = wire::Served {
            name: images::NAME.into(),
            args: json!({ "prompt": "a red circle" }),
            result: String::new(),
            echo: None,
            file: Some(("png".into(), vec![0, 1, 2])),
        };
        let mut said =
            wire::Completion { text: "Here.".into(), served: vec![made], ..Default::default() };
        keep_made(&mut Shelves(kept.clone()), &mut said);

        let kept = kept.lock().unwrap();
        let (name, bytes) = &kept[0];
        assert!(name.starts_with("picture_") && name.ends_with(".png") && bytes == &[0, 1, 2]);
        assert_eq!(said.served[0].result, format!("/imports/{name}"));
        assert_eq!(said.text, format!("Here.\n\n![](/imports/{name})"));
    }

    /// Reads say how many reads there have been.
    struct Shelf(usize);

    impl Tools for Shelf {
        fn schemas(&self) -> Vec<ToolSchema> {
            Vec::new()
        }

        fn call(&mut self, call: &Call) -> ToolOutcome {
            self.0 += 1;
            ToolOutcome::ok(format!("{} v{}", call.args["path"].as_str().unwrap_or(""), self.0))
        }
    }

    fn attached() -> Vec<Mention> {
        vec![Mention { path: "/a.md".into(), id: None }]
    }

    /// An attached note is read once, when its message is sent, and that
    /// reading is what the model keeps: the next request starts as this one.
    #[test]
    fn an_attached_note_is_read_when_its_message_is_sent() {
        let store = MemStore::default();
        let (url, bodies) = mock::serve(vec![sse_text("seen"), sse_text("still")]);
        let d = driver(store.clone(), Shelf(0), url);
        d.send(Cmd::Say { text: "look".into(), mentions: attached() });
        wait_for_run(&d);
        d.send(Cmd::Say { text: "more".into(), mentions: vec![] });
        wait_for_run(&d);

        assert_eq!(kinds(&store.chat()), ["user", "tool", "assistant", "user", "assistant"]);
        let sent: Vec<serde_json::Value> = bodies
            .try_iter()
            .map(|body| serde_json::from_str(&body).unwrap())
            .collect();
        let first = sent[0]["messages"].as_array().unwrap();
        assert_eq!(first.last().unwrap()["content"], "/a.md v1");
        assert_eq!(sent[1]["messages"].as_array().unwrap()[..first.len()], first[..]);
    }

    #[test]
    fn a_message_run_again_reads_its_note_again() {
        let store = MemStore::default();
        let d = driver(store.clone(), Shelf(0), mock::serve(vec![sse_text("a"), sse_text("b")]).0);
        d.send(Cmd::Say { text: "look".into(), mentions: attached() });
        wait_for_run(&d);
        d.send(Cmd::Regenerate);
        wait_for_run(&d);

        let chat = store.chat();
        assert_eq!(kinds(&chat), ["user", "tool", "assistant"]);
        assert!(matches!(&chat.entries[1].body, Body::Tool { result, .. } if result == "/a.md v2"));
    }

    #[test]
    fn a_turn_settles_the_question_and_the_reply() {
        let store = MemStore::default();
        let d = driver(store.clone(), NoTools, mock::serve_once(&sse_text("hi there")));
        d.send(Cmd::Say { text: "hello".into(), mentions: vec![] });
        let events = wait_for_run(&d);
        assert!(events.contains(&Event::Delta("hi there".into())));
        let chat = store.chat();
        assert_eq!(kinds(&chat), ["user", "assistant"]);
        assert_eq!(chat.entries[1].text(), "hi there");
        assert!(!d.busy());
    }

    #[test]
    fn tool_calls_loop_until_the_model_stops_calling() {
        let store = MemStore::default();
        let (url, bodies) =
            mock::serve(vec![sse_call("echo", "{\"text\":\"x\"}"), sse_text("done")]);
        let d = driver(store.clone(), Mock, url);
        d.send(Cmd::Say { text: "go".into(), mentions: vec![] });
        let events = wait_for_run(&d);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, Event::ToolStarted(c) if c.name == "echo"))
        );
        let chat = store.chat();
        assert_eq!(kinds(&chat), ["user", "assistant", "tool", "assistant"]);
        assert_eq!(chat.entries[2].text(), "echo:x");

        let second: serde_json::Value =
            serde_json::from_str(&bodies.iter().nth(1).unwrap()).unwrap();
        let roles: Vec<_> = second["messages"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["role"].as_str().unwrap())
            .collect();
        assert_eq!(roles, ["system", "user", "assistant", "tool"]);
        assert_eq!(second["messages"][3]["content"], "echo:x");
    }

    #[test]
    fn an_abort_ends_the_run_with_an_error_and_no_further_completion() {
        let store = MemStore::default();
        let (url, bodies) = mock::serve(vec![sse_call("wall", "{}")]);
        let d = driver(store.clone(), Mock, url);
        d.send(Cmd::Say { text: "go".into(), mentions: vec![] });
        wait_for_run(&d);
        let chat = store.chat();
        assert_eq!(kinds(&chat), ["user", "assistant", "error"]);
        assert!(chat.entries[2].text().contains("blocked"));
        assert_eq!(bodies.try_iter().count(), 1);
    }

    /// A call that answered the same twice is not run a third time; made a
    /// fourth time, it ends the run.
    #[test]
    fn a_call_repeated_to_the_same_answer_is_refused_and_then_ends_the_run() {
        let store = MemStore::default();
        let call = || sse_call("echo", "{\"text\":\"x\"}");
        let (url, bodies) = mock::serve(vec![call(), call(), call(), call(), sse_text("never")]);
        let d = driver(store.clone(), Mock, url);
        d.send(Cmd::Say { text: "go".into(), mentions: vec![] });
        wait_for_run(&d);

        let chat = store.chat();
        let turns = ["assistant", "tool", "assistant", "tool", "assistant", "tool"];
        assert_eq!(kinds(&chat)[1..7], turns);
        assert_eq!(kinds(&chat)[7..], ["assistant", "error"]);
        assert_eq!((chat.entries[2].text(), chat.entries[4].text()), ("echo:x", "echo:x"));
        assert!(
            matches!(&chat.entries[6].body, Body::Tool { result, ok: false, .. } if result == REPEATED)
        );
        assert!(chat.entries[8].text().starts_with("stopped: echo"));
        assert_eq!(bodies.try_iter().count(), 4);
    }

    #[test]
    fn a_call_made_again_that_answers_differently_is_not_a_repeat() {
        let store = MemStore::default();
        let read = || sse_call("read", "{\"path\":\"/a.md\"}");
        let (url, _) = mock::serve(vec![read(), read(), read(), read(), sse_text("done")]);
        let d = driver(store.clone(), Shelf(0), url);
        d.send(Cmd::Say { text: "go".into(), mentions: vec![] });
        wait_for_run(&d);

        let chat = store.chat();
        assert_eq!(chat.entries.last().unwrap().text(), "done");
        assert!(
            matches!(&chat.entries[8].body, Body::Tool { result, ok: true, .. } if result == "/a.md v4")
        );
    }

    /// Nothing holds the driver any more: a tab navigated away, an app
    /// closing. The call in flight finishes and the run ends there, before
    /// the next call and before another request.
    #[test]
    fn a_dropped_driver_ends_its_run() {
        struct Gated {
            gate: std::sync::mpsc::Receiver<()>,
            calls: Arc<std::sync::atomic::AtomicUsize>,
        }
        impl Tools for Gated {
            fn schemas(&self) -> Vec<ToolSchema> {
                Mock.schemas()
            }
            fn call(&mut self, _: &Call) -> ToolOutcome {
                if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    let _ = self.gate.recv_timeout(Duration::from_secs(10));
                }
                ToolOutcome::ok("did it")
            }
        }
        let two_calls = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
            data: {\"choices\":[{\"delta\":{\"tool_calls\":[\
            {\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"echo\",\"arguments\":\"{}\"}},\
            {\"index\":1,\"id\":\"c2\",\"function\":{\"name\":\"echo\",\"arguments\":\"{}\"}}]}}]}\n\n\
            data: [DONE]\n\n"
            .to_string();
        let store = MemStore::default();
        let (url, bodies) = mock::serve(vec![two_calls, sse_text("never asked for")]);
        let (release, gate) = std::sync::mpsc::channel();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let d = driver(store.clone(), Gated { gate, calls: calls.clone() }, url);
        d.send(Cmd::Say { text: "go".into(), mentions: vec![] });
        wait_for(&d, |e| e.iter().any(|e| matches!(e, Event::ToolStarted(_))));
        drop(d);
        release.send(()).unwrap();

        let start = Instant::now();
        while kinds(&store.chat()).len() < 3 {
            assert!(start.elapsed() < Duration::from_secs(10), "the call in flight never settled");
            std::thread::sleep(Duration::from_millis(10));
        }
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(kinds(&store.chat()), ["user", "assistant", "tool"]);
        assert_eq!(calls.load(Ordering::SeqCst), 1, "the second call never ran");
        assert_eq!(bodies.try_iter().count(), 1, "and nothing more was asked of the provider");
    }

    /// What a provider attaches to a tool call (Gemini's thought signature)
    /// stays on the tool line and goes back with the call on the next
    /// request; another provider never sees it.
    #[test]
    fn what_a_provider_attaches_to_a_call_returns_with_it() {
        let signed =
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
            data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\
            \"extra_content\":{\"google\":{\"thought_signature\":\"SIG\"}},\
            \"function\":{\"name\":\"echo\",\"arguments\":\"{\\\"text\\\":\\\"x\\\"}\"}}]}}]}\n\n\
            data: [DONE]\n\n"
                .to_string();
        let store = MemStore::default();
        let (url, bodies) = mock::serve(vec![signed, sse_text("ok")]);
        let d = driver(store.clone(), Mock, url);
        d.send(Cmd::Say { text: "go".into(), mentions: vec![] });
        wait_for_run(&d);

        let chat = store.chat();
        assert_eq!(kinds(&chat), ["user", "assistant", "tool", "assistant"]);
        let line = chat.entries[2].to_json_line();
        assert!(line.contains("\"thought_signature\":\"SIG\""), "{line}");
        let sent: Vec<String> = bodies.try_iter().collect();
        assert!(!sent[0].contains("SIG"));
        assert!(
            sent[1].contains("\"extra_content\":{\"google\":{\"thought_signature\":\"SIG\"}}"),
            "{}",
            sent[1]
        );

        // The same transcript, folded for a different provider.
        let turns = context::turns(&chat, "u", None, &mut |_| None);
        let req = Request { turns, ..Default::default() };
        let other = Provider {
            name: "other".into(),
            display_name: None,
            needs_key: false,
            kind: Kind::OpenAi,
            base_url: String::new(),
            api_key: None,
            model: "m".into(),
            effort: None,
        };
        assert!(!wire::openai::body(&other, &req).to_string().contains("SIG"));
    }

    /// A provider resolved with an effort asks Claude to think, keeps what it
    /// thought on the tool line, and hands it back in front of the call when
    /// the result goes.
    #[test]
    fn a_thinking_run_hands_its_thinking_back() {
        let thought = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
            data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\",\"signature\":\"\"}}\n\n\
            data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"hm\"}}\n\n\
            data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"SIG\"}}\n\n\
            data: {\"type\":\"content_block_start\",\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"t1\",\"name\":\"echo\"}}\n\n\
            data: {\"type\":\"content_block_delta\",\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"text\\\":\\\"x\\\"}\"}}\n\n\
            data: {\"type\":\"message_stop\"}\n\n"
            .to_string();
        let done = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
            data: {\"type\":\"content_block_start\",\"index\":0,\"content_block\":{\"type\":\"text\"}}\n\n\
            data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"ok\"}}\n\n\
            data: {\"type\":\"message_stop\"}\n\n"
            .to_string();
        let store = MemStore::default();
        let (url, bodies) = mock::serve(vec![thought, done]);
        let config = Config {
            user: "u".into(),
            working_dir: "/".into(),
            provider: Box::new(move || {
                Ok(Provider {
                    name: "claude".into(),
                    display_name: None,
                    needs_key: false,
                    kind: Kind::Anthropic,
                    base_url: url.clone(),
                    api_key: Some("k".into()),
                    model: "m".into(),
                    effort: Some("on".into()),
                })
            }),
            window: |_| None,
        };
        let d = Driver::spawn(store.clone(), Mock, config, || {});
        d.send(Cmd::Say { text: "go".into(), mentions: vec![] });
        assert!(wait_for_run(&d).contains(&Event::Thinking("hm".into())));

        let chat = store.chat();
        assert_eq!(kinds(&chat), ["user", "assistant", "tool", "assistant"]);
        assert!(
            chat.entries[2]
                .to_json_line()
                .contains("\"signature\":\"SIG\"")
        );
        // What it showed of its thinking is on the reply it led to.
        assert!(
            matches!(&chat.entries[1].body, Body::Assistant { thinking, .. } if thinking == "hm")
        );
        let sent: Vec<serde_json::Value> = bodies
            .try_iter()
            .map(|body| serde_json::from_str(&body).unwrap())
            .collect();
        assert_eq!(sent[0]["thinking"]["type"], "enabled");
        assert_eq!(
            sent[1]["messages"][1]["content"][0],
            json!({ "type": "thinking", "thinking": "hm", "signature": "SIG" })
        );
        assert_eq!(sent[1]["messages"][1]["content"][1]["type"], "tool_use");
    }

    /// Replies settle without the whitespace models pad them with; a reply
    /// that was only padding around a tool call settles empty.
    #[test]
    fn replies_settle_trimmed() {
        let store = MemStore::default();
        let padded_call = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
            data: {\"choices\":[{\"delta\":{\"content\":\"  \"}}]}\n\n\
            data: {\"choices\":[{\"delta\":{\"tool_calls\":[{\"index\":0,\"id\":\"c1\",\"function\":{\"name\":\"echo\",\"arguments\":\"{}\"}}]}}]}\n\n\
            data: [DONE]\n\n"
            .to_string();
        let (url, _) = mock::serve(vec![padded_call, sse_text("  42\n")]);
        let d = driver(store.clone(), Mock, url);
        d.send(Cmd::Say { text: "go".into(), mentions: vec![] });
        wait_for_run(&d);
        let chat = store.chat();
        assert_eq!(kinds(&chat), ["user", "assistant", "tool", "assistant"]);
        assert_eq!((chat.entries[1].text(), chat.entries[3].text()), ("", "42"));
    }

    #[test]
    fn a_provider_error_settles_as_an_error_line() {
        let store = MemStore::default();
        let d = driver(
            store.clone(),
            NoTools,
            mock::serve_once(
                "HTTP/1.1 401 Unauthorized\r\nContent-Length: 6\r\nConnection: close\r\n\r\nno key",
            ),
        );
        d.send(Cmd::Say { text: "hello".into(), mentions: vec![] });
        wait_for_run(&d);
        let chat = store.chat();
        assert_eq!(kinds(&chat), ["user", "error"]);
        assert!(chat.entries[1].text().contains("401"));
    }

    #[test]
    fn edit_truncates_and_reruns_from_that_point() {
        let store = MemStore::default();
        let (url, _) = mock::serve(vec![sse_text("one"), sse_text("two")]);
        let d = driver(store.clone(), NoTools, url);
        d.send(Cmd::Say { text: "first".into(), mentions: vec![] });
        wait_for_run(&d);
        let first_id = store.chat().entries[0].id;
        d.send(Cmd::Edit { id: first_id, text: "edited".into(), mentions: vec![] });
        wait_for_run(&d);
        let chat = store.chat();
        assert_eq!(kinds(&chat), ["user", "assistant"]);
        assert_eq!(chat.entries[0].text(), "edited");
        assert_eq!(chat.entries[1].text(), "two");
    }

    /// A turn stopped mid-reply goes on from there when resumed: the note
    /// it attached is not read again, and the model is asked with the
    /// partial reply in place.
    #[test]
    fn a_resumed_turn_goes_on_from_where_it_stopped() {
        let store = MemStore::default();
        const PART: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
            data: {\"choices\":[{\"delta\":{\"content\":\"part\"}}]}\n\n";
        let (url, bodies, release) = mock::serve_held(PART, vec![sse_text("rest")]);
        let d = driver(store.clone(), Shelf(0), url);
        d.send(Cmd::Say { text: "look".into(), mentions: attached() });
        wait_for(&d, |e| e.contains(&Event::Delta("part".into())));
        d.send(Cmd::Stop);
        wait_for_run(&d);
        release.send(()).unwrap();
        assert_eq!(kinds(&store.chat()), ["user", "tool", "assistant"]);

        d.send(Cmd::Resume);
        wait_for_run(&d);
        let chat = store.chat();
        assert_eq!(kinds(&chat), ["user", "tool", "assistant", "assistant"]);
        assert!(
            matches!(&chat.entries[2].body, Body::Assistant { text, interrupted: true, .. } if text == "part")
        );
        assert_eq!(chat.entries[3].text(), "rest");
        let sent: Vec<serde_json::Value> = bodies
            .try_iter()
            .map(|body| serde_json::from_str(&body).unwrap())
            .collect();
        let last = sent[1]["messages"].as_array().unwrap().last().unwrap();
        assert_eq!((&last["role"], &last["content"]), (&json!("assistant"), &json!("part")));
    }

    #[test]
    fn stop_mid_stream_settles_the_partial_as_interrupted() {
        let store = MemStore::default();
        const SLOW: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
            data: {\"choices\":[{\"delta\":{\"content\":\"part\"}}]}\n\n";
        let url = mock::serve_split(SLOW, SLOW.len());
        let d = driver(store.clone(), NoTools, url);
        d.send(Cmd::Say { text: "hello".into(), mentions: vec![] });
        wait_for(&d, |e| e.contains(&Event::Delta("part".into())));
        d.send(Cmd::Stop);
        wait_for_run(&d);
        let chat = store.chat();
        assert_eq!(kinds(&chat), ["user", "assistant"]);
        assert!(
            matches!(&chat.entries[1].body, Body::Assistant { text, interrupted: true, .. } if text == "part")
        );
    }
}
