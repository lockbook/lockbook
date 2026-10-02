//! The turn loop. One driver per chat, on its own thread. A command starts a
//! run: completions with tool calls until the model stops calling tools,
//! each finished piece settled into the store as it lands. Stop cancels the
//! stream and settles what was streamed. The thread is synchronous; only the
//! streaming completion runs on the runtime, so the store and the tools may
//! block freely.

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
use crate::web;
use crate::wire::{self, Call, Piece, Request, images};

/// Bytes of a tool result written to the chat.
pub const TOOL_RESULT_CAP: usize = 16 * 1024;

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
    /// Persist this user's settings without running.
    SetSettings(Settings),
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
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("runtime");
                let client = rt.block_on(async { reqwest::Client::new() });
                Worker {
                    store: Box::new(store),
                    tools: Box::new(tools),
                    config,
                    events: event_tx,
                    wake: Box::new(wake),
                    busy: worker_busy,
                    client,
                    rt,
                    made: Vec::new(),
                    artist: None,
                }
                .run(cmd_rx);
            })
            .expect("spawn chat driver");
        Driver { cmds, events, busy }
    }

    pub fn send(&self, cmd: Cmd) {
        let _ = self.cmds.send(cmd);
    }

    pub fn poll(&self) -> Vec<Event> {
        self.events.try_iter().collect()
    }

    pub fn busy(&self) -> bool {
        self.busy.load(Ordering::Relaxed)
    }
}

struct Worker {
    store: Box<dyn Store>,
    tools: Box<dyn Tools>,
    config: Config,
    events: Sender<Event>,
    wake: Box<dyn Fn() + Send + Sync>,
    busy: Arc<AtomicBool>,
    client: reqwest::Client,
    rt: Runtime,
    made: Vec<Made>,
    /// The provider and model that make pictures for this round's model.
    artist: Option<(Provider, &'static str)>,
}

/// A call made in this run, what it last answered, and how many times
/// running it has since answered the same.
struct Made {
    name: String,
    args: serde_json::Value,
    answer: Option<(String, bool)>,
    repeats: usize,
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
                Cmd::Say { text, mentions } => {
                    self.store.append(said(&user, text, mentions)).map(Some)
                }
                Cmd::Edit { id, text, mentions } => {
                    let entry = said(&user, text, mentions);
                    self.store
                        .update(&mut |chat| {
                            chat.truncate_from(id, &user);
                            chat.push(entry.clone());
                        })
                        .map(|chat| chat.entries.last().cloned())
                }
                Cmd::Regenerate => self
                    .store
                    .update(&mut |chat| {
                        chat.truncate_after_last_user(&user);
                    })
                    .map(|_| None),
                Cmd::SetSettings(settings) => {
                    let saved = self
                        .store
                        .update(&mut |chat| chat.set_settings(&user, settings.clone()));
                    if let Err(err) = saved {
                        self.settle(Entry::error(&user, err));
                    }
                    continue;
                }
                Cmd::Stop => continue,
            };
            match staged {
                Ok(entry) => {
                    if let Some(entry) = entry {
                        self.emit(Event::Written(entry));
                    }
                    self.turn(&mut cmds);
                }
                Err(err) => {
                    self.settle(Entry::error(&user, err));
                }
            }
        }
    }

    fn turn(&mut self, cmds: &mut UnboundedReceiver<Cmd>) {
        self.busy.store(true, Ordering::Relaxed);
        self.emit(Event::RunStarted);
        self.made.clear();
        if self.read_attached(cmds) {
            self.rounds(cmds);
        }
        self.busy.store(false, Ordering::Relaxed);
        self.emit(Event::RunEnded);
    }

    /// Reads what the newest message attached, as tool lines: the model
    /// keeps each note as it was then. Returns whether the run continues.
    fn read_attached(&mut self, cmds: &mut UnboundedReceiver<Cmd>) -> bool {
        let user = self.config.user.clone();
        let Ok((_, bytes)) = self.store.load() else { return true };
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
            let (provider, request) = match self.prepare() {
                Ok(ready) => ready,
                Err(err) => {
                    self.settle(Entry::error(&user, err));
                    break;
                }
            };
            let (heard, outcome) = self.complete(&provider, &request, cmds);
            match outcome {
                Outcome::Stopped => {
                    if !heard.text.trim().is_empty() {
                        self.settle(reply(&user, &provider, &heard, true));
                    }
                    break;
                }
                Outcome::Failed(err) => {
                    self.settle(Entry::error(&user, err));
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
                        .all(|e| self.settle(e))
                    {
                        break;
                    }
                    let said = reply(&user, &provider, &completion, false);
                    if !self.settle(said) || completion.calls.is_empty() {
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
            self.emit(Event::ToolStarted(call.clone()));
            let (text, ok) = match self.call(&call) {
                ToolOutcome::Done { text, ok } => (text, ok),
                ToolOutcome::Abort { text } => {
                    self.settle(Entry::error(&user, format!("stopped: {text}")));
                    return false;
                }
            };
            let result = truncate(&text, TOOL_RESULT_CAP);
            let mut entry = Entry::tool(&user, call.name, call.args, result, ok);
            if let Some(echo) = call.echo.and_then(|e| serde_json::to_value(e).ok()) {
                entry.extra.insert("echo".into(), echo);
            }
            if !self.settle(entry) {
                return false;
            }
            // Told to stop, or nothing holds the driver any more.
            if matches!(cmds.try_recv(), Ok(Cmd::Stop) | Err(TryRecvError::Disconnected)) {
                return false;
            }
        }
        true
    }

    /// Has the provider make the picture `call` asks for, and keeps it.
    fn draw(&mut self, provider: &Provider, model: &str, call: &Call) -> ToolOutcome {
        let prompt = call.args["prompt"].as_str().unwrap_or_default();
        let drawing = images::generate(&self.client, provider, model, prompt);
        let kept = self
            .rt
            .block_on(drawing)
            .and_then(|(ext, bytes)| self.tools.keep(&picture_name(&ext), &bytes));
        match kept {
            Ok(path) => ToolOutcome::ok(path),
            Err(e) => ToolOutcome::err(e),
        }
    }

    /// Runs `call` unless it has answered the same twice in this run: then
    /// it is refused once, and after that it ends the run.
    fn call(&mut self, call: &Call) -> ToolOutcome {
        let same = |m: &Made| m.name == call.name && m.args == call.args;
        let at = self.made.iter().position(same).unwrap_or_else(|| {
            let (name, args) = (call.name.clone(), call.args.clone());
            self.made
                .push(Made { name, args, answer: None, repeats: 0 });
            self.made.len() - 1
        });
        match self.made[at].repeats {
            0 => {}
            1 => {
                self.made[at].repeats = 2;
                return ToolOutcome::err(REPEATED);
            }
            _ => return ToolOutcome::Abort { text: format!("{} kept repeating", call.name) },
        }
        let outcome = match self.artist.clone().filter(|_| call.name == images::NAME) {
            Some((provider, model)) => self.draw(&provider, model, call),
            None => self.tools.call(call),
        };
        if let ToolOutcome::Done { text, ok } = &outcome {
            let answer = Some((text.clone(), *ok));
            if self.made[at].answer == answer {
                self.made[at].repeats = 1;
            } else {
                // A new answer is progress: earlier repeats no longer count.
                self.made.iter_mut().for_each(|m| m.repeats = 0);
                self.made[at].answer = answer;
            }
        }
        outcome
    }

    fn prepare(&mut self) -> Result<(Provider, Request), String> {
        let (_, bytes) = self.store.load()?;
        let chat = Chat::parse(&bytes);
        let provider = (self.config.provider)()?;
        if provider.needs_key {
            return Err(format!("{} needs an API key", provider.label()));
        }
        let settings = chat.settings_for(&self.config.user);
        let territory = Territory::new(&self.config.working_dir, &settings);
        self.tools
            .prepare(&chat, &self.config.user, &self.config.working_dir);
        let instructions = self.tools.instructions(&self.config.working_dir);
        let system = context::system_prompt(&territory, &instructions);
        let mut tools = self.tools.schemas();
        if provider.reaches_the_web() {
            tools.retain(|tool| tool.name != web::SEARCH && tool.name != web::FETCH);
        }
        self.artist = provider.draws().map(|model| (provider.clone(), model));
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
                self.emit(Event::Delta(text));
            }
            Piece::Thinking(text) => {
                heard.thinking.push_str(&text);
                self.emit(Event::Thinking(text));
            }
        }
    }

    /// Appends `entry`; false means it was lost and the run should stop.
    fn settle(&mut self, entry: Entry) -> bool {
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

    fn emit(&self, event: Event) {
        let _ = self.events.send(event);
        (self.wake)();
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
