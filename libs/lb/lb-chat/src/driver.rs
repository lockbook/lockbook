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
use lb_rs::model::chat::{Body, Chat, Entry, Mention};
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tracing::warn;

use crate::context::{self, truncate};
use crate::provider::Provider;
use crate::store::Store;
use crate::tools::Tools;
use crate::wire::{self, Call, Request};

/// Bytes of a tool result written to the chat.
pub const TOOL_RESULT_CAP: usize = 16 * 1024;

pub enum Cmd {
    Say {
        text: String,
        mentions: Vec<Mention>,
    },
    /// Replace the message `id` and everything after it, then run.
    Edit {
        id: Uuid,
        text: String,
    },
    /// Drop everything after the last message and run again.
    Regenerate,
    Stop,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    RunStarted,
    Delta(String),
    ToolStarted(Call),
    /// A line settled into the chat.
    Written(Entry),
    RunEnded,
}

pub struct Config {
    pub user: String,
    pub working_dir: String,
    pub provider: Box<dyn Fn() -> Result<Provider, String> + Send>,
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
                    let mut entry = Entry::user(&user, text);
                    if let Body::User { mentions: m, .. } = &mut entry.body {
                        *m = mentions;
                    }
                    self.store.append(entry).map(Some)
                }
                Cmd::Edit { id, text } => {
                    let entry = Entry::user(&user, text);
                    self.store
                        .update(&mut |chat| {
                            chat.truncate_from(id);
                            chat.push(entry.clone());
                        })
                        .map(|chat| chat.entries.last().cloned())
                }
                Cmd::Regenerate => self
                    .store
                    .update(&mut |chat| {
                        let last_user = chat
                            .entries
                            .iter()
                            .rposition(|e| matches!(e.body, Body::User { .. }));
                        if let Some(i) = last_user {
                            chat.entries.truncate(i + 1);
                        }
                    })
                    .map(|_| None),
                Cmd::Stop => continue,
            };
            match staged {
                Ok(entry) => {
                    if let Some(entry) = entry {
                        self.emit(Event::Written(entry));
                    }
                    self.turn(&mut cmds);
                }
                Err(err) => self.settle(Entry::error(&user, err)),
            }
        }
    }

    fn turn(&mut self, cmds: &mut UnboundedReceiver<Cmd>) {
        self.busy.store(true, Ordering::Relaxed);
        self.emit(Event::RunStarted);
        let user = self.config.user.clone();
        loop {
            let (provider, request) = match self.prepare() {
                Ok(ready) => ready,
                Err(err) => {
                    self.settle(Entry::error(&user, err));
                    break;
                }
            };
            let (streamed, outcome) = self.complete(&provider, &request, cmds);
            match outcome {
                Outcome::Stopped => {
                    if !streamed.is_empty() {
                        let mut entry = Entry::assistant(
                            &user,
                            streamed,
                            provider.selection(),
                            Default::default(),
                        );
                        if let Body::Assistant { interrupted, .. } = &mut entry.body {
                            *interrupted = true;
                        }
                        self.settle(entry);
                    }
                    break;
                }
                Outcome::Failed(err) => {
                    self.settle(Entry::error(&user, err));
                    break;
                }
                Outcome::Finished(completion) => {
                    self.settle(Entry::assistant(
                        &user,
                        completion.text,
                        provider.selection(),
                        completion.usage,
                    ));
                    if completion.calls.is_empty() {
                        break;
                    }
                    let mut stopped = false;
                    for call in completion.calls {
                        self.emit(Event::ToolStarted(call.clone()));
                        let outcome = self.tools.call(&call);
                        let result = truncate(&outcome.text, TOOL_RESULT_CAP);
                        self.settle(Entry::tool(&user, call.name, call.args, result, outcome.ok));
                        if matches!(cmds.try_recv(), Ok(Cmd::Stop)) {
                            stopped = true;
                            break;
                        }
                    }
                    if stopped {
                        break;
                    }
                }
            }
        }
        self.busy.store(false, Ordering::Relaxed);
        self.emit(Event::RunEnded);
    }

    fn prepare(&mut self) -> Result<(Provider, Request), String> {
        let (_, bytes) = self.store.load()?;
        let chat = Chat::parse(&bytes);
        let provider = (self.config.provider)()?;
        let tools = &mut self.tools;
        let turns = context::turns(&chat, &mut |m| tools.read_mention(m));
        let request = Request {
            system: context::system_prompt(&self.config.working_dir),
            turns,
            tools: self.tools.schemas(),
        };
        Ok((provider, request))
    }

    fn complete(
        &self, provider: &Provider, request: &Request, cmds: &mut UnboundedReceiver<Cmd>,
    ) -> (String, Outcome) {
        let (delta_tx, mut deltas) = unbounded_channel::<String>();
        let mut streamed = String::new();
        let outcome = self.rt.block_on(async {
            let future = wire::complete(&self.client, provider, request, &delta_tx);
            tokio::pin!(future);
            loop {
                tokio::select! {
                    result = &mut future => break match result {
                        Ok(completion) => Outcome::Finished(completion),
                        Err(err) => Outcome::Failed(err),
                    },
                    Some(delta) = deltas.recv() => {
                        streamed.push_str(&delta);
                        self.emit(Event::Delta(delta));
                    }
                    cmd = cmds.recv() => match cmd {
                        Some(Cmd::Stop) | None => break Outcome::Stopped,
                        Some(_) => warn!("chat command ignored while a run is live"),
                    },
                }
            }
        });
        while let Ok(delta) = deltas.try_recv() {
            streamed.push_str(&delta);
            self.emit(Event::Delta(delta));
        }
        (streamed, outcome)
    }

    fn settle(&mut self, entry: Entry) {
        match self.store.append(entry) {
            Ok(stored) => self.emit(Event::Written(stored)),
            Err(err) => warn!("chat write failed: {err}"),
        }
    }

    fn emit(&self, event: Event) {
        let _ = self.events.send(event);
        (self.wake)();
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use serde_json::json;

    use super::*;
    use crate::mock::{self, sse_call, sse_text};
    use crate::provider::Kind;
    use crate::store::MemStore;
    use crate::tools::{NoTools, ToolOutcome};
    use crate::wire::ToolSchema;

    fn driver(store: MemStore, tools: impl Tools + 'static, base_url: String) -> Driver {
        let config = Config {
            user: "u".into(),
            working_dir: "/".into(),
            provider: Box::new(move || {
                Ok(Provider {
                    name: "mock".into(),
                    kind: Kind::OpenAi,
                    base_url: base_url.clone(),
                    api_key: None,
                    model: "m".into(),
                })
            }),
        };
        Driver::spawn(store, tools, config, || {})
    }

    fn wait_for_run(driver: &Driver) -> Vec<Event> {
        let start = Instant::now();
        let mut events = Vec::new();
        while start.elapsed() < Duration::from_secs(10) {
            events.extend(driver.poll());
            if events.contains(&Event::RunEnded) {
                return events;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("run never ended: {events:?}");
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

    struct Echo;

    impl Tools for Echo {
        fn schemas(&self) -> Vec<ToolSchema> {
            vec![ToolSchema {
                name: "echo".into(),
                description: "echoes".into(),
                parameters: json!({"type": "object", "properties": {"text": {"type": "string"}}}),
            }]
        }

        fn call(&mut self, call: &Call) -> ToolOutcome {
            ToolOutcome::ok(format!("echo:{}", call.args["text"].as_str().unwrap_or("")))
        }
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
        let d = driver(store.clone(), Echo, url);
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
        d.send(Cmd::Edit { id: first_id, text: "edited".into() });
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
        let start = Instant::now();
        while !d.poll().contains(&Event::Delta("part".into())) {
            assert!(start.elapsed() < Duration::from_secs(10), "no delta");
            std::thread::sleep(Duration::from_millis(10));
        }
        d.send(Cmd::Stop);
        wait_for_run(&d);
        let chat = store.chat();
        assert_eq!(kinds(&chat), ["user", "assistant"]);
        assert!(
            matches!(&chat.entries[1].body, Body::Assistant { text, interrupted: true, .. } if text == "part")
        );
    }
}
