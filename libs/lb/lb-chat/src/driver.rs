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
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};
use tracing::warn;

use crate::context::{self, truncate};
use crate::provider::Provider;
use crate::store::Store;
use crate::territory::Territory;
use crate::tools::{ToolOutcome, Tools};
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
    /// Answer the pending [`Event::Ask`].
    Approve,
    Deny,
    /// Persist this user's settings without running.
    SetSettings(Settings),
    Stop,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    RunStarted,
    Delta(String),
    ToolStarted(Call),
    /// A tool needs the user's go-ahead; the run waits for Approve or Deny.
    Ask {
        call: Call,
        prompt: String,
    },
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
                Cmd::Approve | Cmd::Deny | Cmd::Stop => continue,
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
                    let reply = Entry::assistant(
                        &user,
                        completion.text,
                        provider.selection(),
                        completion.usage,
                    );
                    if !self.settle(reply) || completion.calls.is_empty() {
                        break;
                    }
                    if !self.run_tools(completion.calls, cmds) {
                        break;
                    }
                }
            }
        }
        self.busy.store(false, Ordering::Relaxed);
        self.emit(Event::RunEnded);
    }

    /// Runs the calls in order. Returns whether the run continues.
    fn run_tools(&mut self, calls: Vec<Call>, cmds: &mut UnboundedReceiver<Cmd>) -> bool {
        let user = self.config.user.clone();
        for call in calls {
            self.emit(Event::ToolStarted(call.clone()));
            let mut outcome = self.tools.call(&call, false);
            if let ToolOutcome::Ask { prompt } = &outcome {
                self.emit(Event::Ask { call: call.clone(), prompt: prompt.clone() });
                outcome = loop {
                    match cmds.blocking_recv() {
                        Some(Cmd::Approve) => break self.tools.call(&call, true),
                        Some(Cmd::Deny) => break ToolOutcome::err("the user declined"),
                        Some(Cmd::Stop) | None => return false,
                        Some(_) => warn!("chat command ignored while a tool awaits approval"),
                    }
                };
            }
            let (text, ok) = match outcome {
                ToolOutcome::Done { text, ok } => (text, ok),
                ToolOutcome::Ask { .. } => ("the user declined".to_string(), false),
                ToolOutcome::Grant { path, text } => {
                    let granted = self.store.update(&mut |chat| {
                        let mut settings = chat.settings_for(&user);
                        if !settings.include.contains(&path) {
                            settings.include.push(path.clone());
                        }
                        chat.set_settings(&user, settings);
                    });
                    match granted {
                        Ok(_) => (text, true),
                        Err(err) => (err, false),
                    }
                }
                ToolOutcome::Abort { text } => {
                    self.settle(Entry::error(&user, format!("stopped: {text}")));
                    return false;
                }
            };
            let result = truncate(&text, TOOL_RESULT_CAP);
            if !self.settle(Entry::tool(&user, call.name, call.args, result, ok)) {
                return false;
            }
            if matches!(cmds.try_recv(), Ok(Cmd::Stop)) {
                return false;
            }
        }
        true
    }

    fn prepare(&mut self) -> Result<(Provider, Request), String> {
        let (_, bytes) = self.store.load()?;
        let chat = Chat::parse(&bytes);
        let provider = (self.config.provider)()?;
        let settings = chat.settings_for(&self.config.user);
        let territory = Territory::new(&self.config.working_dir, &settings);
        self.tools
            .prepare(&chat, &self.config.user, &self.config.working_dir);
        let tools = &mut self.tools;
        let turns = context::turns(&chat, &self.config.user, &mut |m| tools.read_mention(m));
        let request = Request {
            system: context::system_prompt(&territory),
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

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use serde_json::json;

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
                    kind: Kind::OpenAi,
                    base_url: base_url.clone(),
                    api_key: None,
                    model: "m".into(),
                })
            }),
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

    /// `echo` answers; `danger` asks first; `wall` aborts; `grant` grants.
    struct Mock;

    impl Tools for Mock {
        fn schemas(&self) -> Vec<ToolSchema> {
            ["echo", "danger", "wall", "grant"]
                .iter()
                .map(|name| ToolSchema {
                    name: name.to_string(),
                    description: name.to_string(),
                    parameters: json!({"type": "object", "properties": {"text": {"type": "string"}}}),
                })
                .collect()
        }

        fn call(&mut self, call: &Call, approved: bool) -> ToolOutcome {
            match call.name.as_str() {
                "echo" => {
                    ToolOutcome::ok(format!("echo:{}", call.args["text"].as_str().unwrap_or("")))
                }
                "danger" if !approved => ToolOutcome::Ask { prompt: "ok?".into() },
                "danger" => ToolOutcome::ok("did it"),
                "wall" => ToolOutcome::Abort { text: "blocked".into() },
                "grant" => {
                    ToolOutcome::Grant { path: "/more/".into(), text: "granted /more/".into() }
                }
                _ => ToolOutcome::err("unknown"),
            }
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
    fn approval_runs_the_tool_and_denial_answers_the_model() {
        for (decision, expected) in [(Cmd::Approve, "did it"), (Cmd::Deny, "the user declined")] {
            let store = MemStore::default();
            let (url, _) = mock::serve(vec![sse_call("danger", "{}"), sse_text("ok")]);
            let d = driver(store.clone(), Mock, url);
            d.send(Cmd::Say { text: "go".into(), mentions: vec![] });
            let events = wait_for(&d, |e| e.iter().any(|e| matches!(e, Event::Ask { .. })));
            assert!(
                events
                    .iter()
                    .any(|e| matches!(e, Event::Ask { prompt, .. } if prompt == "ok?"))
            );
            d.send(decision);
            wait_for_run(&d);
            let chat = store.chat();
            assert_eq!(kinds(&chat), ["user", "assistant", "tool", "assistant"]);
            assert_eq!(chat.entries[2].text(), expected);
        }
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

    #[test]
    fn a_grant_widens_the_users_settings() {
        let store = MemStore::default();
        let (url, _) = mock::serve(vec![sse_call("grant", "{}"), sse_text("ok")]);
        let d = driver(store.clone(), Mock, url);
        d.send(Cmd::Say { text: "go".into(), mentions: vec![] });
        wait_for_run(&d);
        let chat = store.chat();
        assert_eq!(kinds(&chat), ["user", "assistant", "tool", "assistant"]);
        assert_eq!(chat.settings_for("u").include, ["/more/"]);
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
