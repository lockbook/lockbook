//! A spoken conversation: the realtime socket's events folded into chat
//! lines, each written once it is settled and in the order it happened.
//! The server owns the floor: it decides when the user has spoken and
//! cancels its own reply when they talk over it. What is ours is what the
//! user heard: audio arrives well ahead of its playing, so a reply cut
//! short is written down as far as it was played, and the server is told
//! the same.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use lb_rs::model::chat::{Body, Chat, Entry, Usage};
use tokio::sync::mpsc::UnboundedReceiver;
use tokio::task::block_in_place;
use tracing::warn;

use crate::context::truncate;
use crate::driver::{Cmd, Event, Lines, Made, TOOL_RESULT_CAP};
use crate::provider::Provider;
use crate::tools::{ToolOutcome, Tools};
use crate::wire::realtime::{self, BYTES_PER_MS, Incoming, Socket};
use crate::wire::{Call, Request};

/// Written as the user's words when they could not be made out.
pub const UNHEARD: &str = "(not transcribed)";
/// Said to the model after the chat's system prompt.
const SPOKEN: &str =
    "You are talking with the user by voice: speak plainly and briefly, with no markdown.";
/// How long a session being closed waits for the words of a turn the user
/// just finished.
const LAST_WORDS: Duration = Duration::from_secs(2);

/// What the driver lends the session. The store and the tools block, so
/// they are used from outside the runtime.
pub(crate) struct Host<'a> {
    pub user: &'a str,
    /// What the reply lines are stamped with.
    pub model: String,
    pub working_dir: &'a str,
    pub lines: &'a Lines,
    pub tools: &'a mut dyn Tools,
    pub made: &'a mut Made,
    pub busy: &'a AtomicBool,
}

impl Host<'_> {
    fn settle(&self, entry: Entry) -> bool {
        block_in_place(|| self.lines.settle(entry))
    }
}

/// A line's place in the chat, and what settles it.
enum Slot {
    Ready(Entry),
    /// The user's words for turn `item`, once transcribed.
    Heard(String),
    /// Reply `n` of the session, once played out or cut short.
    Reply(usize),
}

/// A reply as it is spoken: what has arrived, what has played, and the
/// words in step with the audio.
#[derive(Default)]
struct Reply {
    item: String,
    /// Milliseconds of audio received, and played.
    received: u64,
    played: u64,
    /// Each run of words with the audio received when it came.
    words: Vec<(u64, String)>,
    /// Runs of words told so far, as they came within what has played.
    told: usize,
    done: bool,
    /// Where the user cut in, in milliseconds played.
    cut: Option<u64>,
    usage: Usage,
}

impl Reply {
    fn settled(&self) -> bool {
        self.done && (self.cut.is_some() || self.played >= self.received)
    }

    /// The words the user heard: nothing of a reply cut before any of it
    /// played, since the first words come ahead of their audio.
    fn heard(&self) -> String {
        let within = match self.cut {
            Some(0) => return String::new(),
            Some(cut) => cut,
            None => u64::MAX,
        };
        self.words
            .iter()
            .filter(|(at, _)| *at <= within)
            .map(|(_, words)| words.as_str())
            .collect()
    }
}

struct Session<'a> {
    host: Host<'a>,
    socket: Socket,
    queue: VecDeque<Slot>,
    replies: Vec<Reply>,
    /// A response is being made, or has been asked for.
    responding: bool,
    in_run: bool,
    /// A line could not be written; the session ends.
    lost: bool,
}

/// Holds the conversation until told to stop or the socket goes.
pub(crate) async fn run(
    host: Host<'_>, provider: &Provider, model: &str, request: Request,
    cmds: &mut UnboundedReceiver<Cmd>,
) {
    let opened = async {
        let mut socket = Socket::connect(provider, model).await?;
        let instructions = format!("{}\n\n{SPOKEN}", request.system);
        socket
            .send(realtime::session(&instructions, &request.tools))
            .await?;
        for item in realtime::items(&request.turns) {
            socket.send(item).await?;
        }
        Ok::<_, String>(socket)
    };
    let socket = match opened.await {
        Ok(socket) => socket,
        Err(e) => {
            host.settle(Entry::error(host.user, format!("voice: {e}")));
            host.lines.emit(Event::VoiceEnded);
            return;
        }
    };
    let mut session = Session {
        host,
        socket,
        queue: VecDeque::new(),
        replies: Vec::new(),
        responding: false,
        in_run: false,
        lost: false,
    };
    let outcome = loop {
        if session.lost {
            break Ok(());
        }
        let step = tokio::select! {
            incoming = session.socket.next() => match incoming {
                Some(Ok(incoming)) => session.take(incoming).await,
                Some(Err(e)) => break Err(e),
                None => break Err("the connection closed".to_string()),
            },
            cmd = cmds.recv() => match cmd {
                Some(Cmd::Audio(pcm)) => session.socket.send(realtime::audio(&pcm)).await,
                Some(Cmd::Played { reply, ms }) => {
                    session.played(reply, ms);
                    Ok(())
                }
                Some(Cmd::Stop) | None => break Ok(()),
                Some(_) => {
                    warn!("chat command ignored while voice is live");
                    Ok(())
                }
            },
        };
        if let Err(e) = step {
            break Err(e);
        }
    };
    session.end(outcome).await;
}

impl Session<'_> {
    async fn take(&mut self, incoming: Incoming) -> Result<(), String> {
        match incoming {
            Incoming::Ready => self.host.lines.emit(Event::VoiceStarted),
            Incoming::SpeechStarted => self.interrupt().await?,
            Incoming::Committed { item } => self.queue.push_back(Slot::Heard(item)),
            Incoming::Hearing { text, .. } => self.host.lines.emit(Event::Hearing(text)),
            Incoming::Heard { item, text } => {
                let slot = self
                    .queue
                    .iter_mut()
                    .find(|slot| matches!(slot, Slot::Heard(turn) if *turn == item));
                if let Some(slot) = slot {
                    let text = text
                        .filter(|text| !text.trim().is_empty())
                        .unwrap_or_else(|| UNHEARD.to_string());
                    *slot = Slot::Ready(said(self.host.user, text));
                }
            }
            Incoming::ResponseCreated => {
                self.responding = true;
                if !self.in_run {
                    self.in_run = true;
                    self.host.made.clear();
                    self.host.busy.store(true, Ordering::Relaxed);
                    self.host.lines.emit(Event::RunStarted);
                }
            }
            Incoming::Audio { item, pcm } => {
                let n = self.reply(&item);
                // What comes after the cut was never heard.
                if self.replies[n].cut.is_none() {
                    self.replies[n].received += pcm.len() as u64 / BYTES_PER_MS;
                    self.host
                        .lines
                        .emit(Event::Audio { reply: n as u32 + 1, pcm });
                }
            }
            Incoming::Transcript { item, text } => {
                let n = self.reply(&item);
                let reply = &mut self.replies[n];
                reply.words.push((reply.received, text));
                self.tell(n);
            }
            Incoming::ResponseDone { status, detail, spoken, calls, usage } => {
                self.responding = false;
                if status == "failed" {
                    let error = Entry::error(self.host.user, format!("voice: {detail}"));
                    self.queue.push_back(Slot::Ready(error));
                }
                for item in &spoken {
                    let n = self.reply(item);
                    self.replies[n].done = true;
                    self.replies[n].usage = usage;
                }
                // A response cut short may hold a call with half its arguments.
                let calls = if status == "completed" { calls } else { Vec::new() };
                // A round of calls alone is a reply with nothing said.
                if spoken.is_empty() && !calls.is_empty() {
                    let mut entry = Entry::assistant(self.host.user, "", &self.host.model, usage);
                    if let Body::Assistant { spoken, .. } = &mut entry.body {
                        *spoken = true;
                    }
                    self.queue.push_back(Slot::Ready(entry));
                }
                self.run_tools(calls).await?;
            }
            // A response asked for while the server had begun one of its
            // own is refused; the server's goes on, and nothing is lost.
            Incoming::Error(message) if message.contains("active response") => {
                warn!("voice: {message}");
            }
            Incoming::Error(message) => {
                let error = Entry::error(self.host.user, format!("voice: {message}"));
                self.queue.push_back(Slot::Ready(error));
            }
        }
        self.flush();
        Ok(())
    }

    /// The user spoke over what was playing: every reply not played out is
    /// cut where it was, and the server is told what was heard.
    async fn interrupt(&mut self) -> Result<(), String> {
        let first = self.replies.iter().position(|reply| !reply.settled());
        let Some(first) = first else { return Ok(()) };
        for reply in &mut self.replies[first..] {
            if reply.cut.is_none() {
                let at = reply.played.min(reply.received);
                reply.cut = Some(at);
                self.socket
                    .send(realtime::truncate(&reply.item, at))
                    .await?;
            }
        }
        self.host.lines.emit(Event::Interrupted);
        Ok(())
    }

    /// Runs the calls in order, posting each answer, then asks for a
    /// response unless one is already on its way.
    async fn run_tools(&mut self, calls: Vec<Call>) -> Result<(), String> {
        if calls.is_empty() {
            return Ok(());
        }
        let Host { lines, tools, made, user, working_dir, .. } = &mut self.host;
        block_in_place(|| {
            if let Ok((_, bytes)) = lines.store.load() {
                tools.prepare(&Chat::parse(&bytes), user, working_dir);
            }
        });
        for call in calls {
            lines.emit(Event::ToolStarted(call.clone()));
            let outcome = made.call(&call, |call| block_in_place(|| tools.call(call)));
            let (text, ok) = match outcome {
                ToolOutcome::Done { text, ok } => (text, ok),
                ToolOutcome::Abort { text } => {
                    let error = Entry::error(*user, format!("stopped: {text}"));
                    self.queue.push_back(Slot::Ready(error));
                    return Ok(());
                }
            };
            let result = truncate(&text, TOOL_RESULT_CAP);
            self.socket
                .send(realtime::output(&call.id, &result))
                .await?;
            let entry = Entry::tool(*user, call.name, call.args, result, ok);
            self.queue.push_back(Slot::Ready(entry));
        }
        if !self.responding {
            self.socket.send(realtime::respond()).await?;
            self.responding = true;
        }
        Ok(())
    }

    /// The client has played reply `reply` up to `ms`.
    fn played(&mut self, reply: u32, ms: u64) {
        let n = reply as usize;
        if n == 0 || n > self.replies.len() {
            return;
        }
        let reply = &mut self.replies[n - 1];
        reply.played = reply.played.max(ms);
        self.tell(n - 1);
        self.flush();
    }

    /// Tells the words of reply `n` that have played and not been told.
    fn tell(&mut self, n: usize) {
        let reply = &mut self.replies[n];
        let within = reply.cut.map_or(reply.played, |cut| cut.min(reply.played));
        while let Some((at, words)) = reply.words.get(reply.told) {
            if *at > within {
                break;
            }
            reply.told += 1;
            self.host.lines.emit(Event::Delta(words.clone()));
        }
    }

    /// The reply for `item`, met now if never before.
    fn reply(&mut self, item: &str) -> usize {
        match self.replies.iter().position(|reply| reply.item == item) {
            Some(n) => n,
            None => {
                self.replies
                    .push(Reply { item: item.to_string(), ..Default::default() });
                self.queue.push_back(Slot::Reply(self.replies.len() - 1));
                self.replies.len() - 1
            }
        }
    }

    /// Writes the lines at the front of the queue that are settled, and
    /// ends the run once nothing of it is left to settle.
    fn flush(&mut self) {
        loop {
            let entry = match self.queue.front() {
                None | Some(Slot::Heard(_)) => break,
                Some(Slot::Ready(_)) => match self.queue.pop_front() {
                    Some(Slot::Ready(entry)) => entry,
                    _ => unreachable!(),
                },
                Some(Slot::Reply(n)) => {
                    let reply = &self.replies[*n];
                    if !reply.settled() {
                        break;
                    }
                    let (text, cut, usage) = (reply.heard(), reply.cut.is_some(), reply.usage);
                    self.queue.pop_front();
                    // Nothing was heard of it.
                    if text.trim().is_empty() {
                        continue;
                    }
                    let mut entry =
                        Entry::assistant(self.host.user, text.trim(), &self.host.model, usage);
                    if let Body::Assistant { spoken, interrupted, .. } = &mut entry.body {
                        *spoken = true;
                        *interrupted = cut;
                    }
                    entry
                }
            };
            if !self.host.settle(entry) {
                self.lost = true;
                return;
            }
        }
        let replying = self.queue.iter().any(|slot| matches!(slot, Slot::Reply(_)));
        if self.in_run && !self.responding && !replying {
            self.in_run = false;
            self.host.busy.store(false, Ordering::Relaxed);
            self.host.lines.emit(Event::RunEnded);
        }
    }

    /// Takes in what the server says until the turn just spoken has its
    /// words.
    async fn last_words(&mut self) {
        while self.queue.iter().any(|s| matches!(s, Slot::Heard(_))) {
            match self.socket.next().await {
                Some(Ok(Incoming::SpeechStarted)) => {}
                Some(Ok(incoming)) => {
                    let _ = self.take(incoming).await;
                }
                _ => break,
            }
        }
    }

    /// Settles what stands: the words of a turn just finished are waited
    /// for a moment, a reply still playing is cut where it is.
    async fn end(mut self, outcome: Result<(), String>) {
        if outcome.is_ok() {
            let _ = tokio::time::timeout(LAST_WORDS, self.last_words()).await;
        }
        for slot in &mut self.queue {
            if let Slot::Heard(_) = slot {
                *slot = Slot::Ready(said(self.host.user, UNHEARD.to_string()));
            }
        }
        for reply in &mut self.replies {
            if !reply.settled() {
                reply.cut = Some(reply.played.min(reply.received));
                reply.done = true;
            }
        }
        self.responding = false;
        self.flush();
        if let Err(e) = outcome {
            if !self.lost {
                self.host
                    .settle(Entry::error(self.host.user, format!("voice: {e}")));
            }
        }
        if self.in_run {
            self.host.busy.store(false, Ordering::Relaxed);
            self.host.lines.emit(Event::RunEnded);
        }
        self.host.lines.emit(Event::VoiceEnded);
        self.socket.close().await;
    }
}

/// The line for what the user said.
fn said(user: &str, text: String) -> Entry {
    let mut entry = Entry::user(user, text);
    if let Body::User { spoken, .. } = &mut entry.body {
        *spoken = true;
    }
    entry
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use lb_rs::model::chat::Usage;
    use serde_json::{Value, json};

    use super::*;
    use crate::driver::{Config, Driver};
    use crate::mock::{Step, serve_ws};
    use crate::provider::Kind;
    use crate::store::{MemStore, Store};
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
                    api_key: Some("k".into()),
                    model: "gpt-realtime".into(),
                    effort: None,
                })
            }),
            window: |_| None,
        };
        Driver::spawn(store, tools, config, || {})
    }

    /// Polls until `done` holds of everything seen so far.
    fn wait_for(driver: &Driver, seen: &mut Vec<Event>, done: impl Fn(&[Event]) -> bool) {
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(10) {
            seen.extend(driver.poll());
            if done(seen) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("timed out: {seen:?}");
    }

    fn count(seen: &[Event], like: impl Fn(&Event) -> bool) -> usize {
        seen.iter().filter(|e| like(e)).count()
    }

    fn opened() -> Vec<Step> {
        vec![
            Step::Expect("session.update"),
            Step::Send(json!({ "type": "session.updated" })),
            Step::Expect("input_audio_buffer.append"),
        ]
    }

    fn committed(item: &str) -> Step {
        Step::Send(json!({ "type": "input_audio_buffer.committed", "item_id": item }))
    }

    fn heard(item: &str, text: &str) -> Step {
        Step::Send(json!({
            "type": "conversation.item.input_audio_transcription.completed",
            "item_id": item, "transcript": text,
        }))
    }

    fn created() -> Step {
        Step::Send(json!({ "type": "response.created", "response": { "id": "r" } }))
    }

    /// `ms` of silence of reply `item`.
    fn audio(item: &str, ms: usize) -> Step {
        let pcm = vec![0u8; ms * BYTES_PER_MS as usize];
        Step::Send(json!({
            "type": "response.output_audio.delta", "item_id": item, "delta": base64::encode(pcm),
        }))
    }

    fn words(item: &str, text: &str) -> Step {
        Step::Send(json!({
            "type": "response.output_audio_transcript.delta", "item_id": item, "delta": text,
        }))
    }

    fn message(item: &str) -> Value {
        json!({ "type": "message", "id": item, "role": "assistant" })
    }

    fn done(status: &str, output: Vec<Value>, usage: (u64, u64)) -> Step {
        Step::Send(json!({
            "type": "response.done",
            "response": {
                "status": status,
                "status_details": { "reason": "turn_detected" },
                "output": output,
                "usage": { "input_tokens": usage.0, "output_tokens": usage.1 },
            }
        }))
    }

    fn chunk() -> Cmd {
        Cmd::Audio(vec![0; 4800])
    }

    fn kinds(chat: &Chat) -> Vec<String> {
        chat.entries
            .iter()
            .map(|e| match &e.body {
                Body::User { text, spoken, .. } => {
                    format!("user{}:{text}", if *spoken { "~" } else { "" })
                }
                Body::Assistant { text, spoken, interrupted, .. } => {
                    let cut = if *interrupted { "!" } else { "" };
                    format!("assistant{}{cut}:{text}", if *spoken { "~" } else { "" })
                }
                Body::Tool { name, result, .. } => format!("tool:{name}:{result}"),
                Body::Error { text } => format!("error:{text}"),
                Body::Other(_) => "other".into(),
            })
            .collect()
    }

    /// The user's words settle when transcribed, and the reply when it has
    /// played: its words are told as they play, not as they arrive.
    #[test]
    fn a_spoken_turn_settles_what_was_said_and_then_what_was_heard() {
        let mut steps = opened();
        steps.extend([
            Step::Send(json!({ "type": "input_audio_buffer.speech_started" })),
            committed("u1"),
            created(),
            audio("a1", 100),
            words("a1", "Hello"),
            audio("a1", 100),
            words("a1", " there"),
            heard("u1", "hi"),
            done("completed", vec![message("a1")], (7, 3)),
        ]);
        let (url, sent) = serve_ws(steps);
        let store = MemStore::default();
        let d = driver(store.clone(), NoTools, url);
        let mut seen = Vec::new();
        d.send(Cmd::StartVoice);
        wait_for(&d, &mut seen, |e| e.contains(&Event::VoiceStarted));
        d.send(chunk());
        wait_for(&d, &mut seen, |e| {
            count(e, |e| matches!(e, Event::Audio { reply: 1, .. })) == 2
                && count(e, |e| matches!(e, Event::Written(_))) == 1
        });
        assert_eq!(kinds(&store.chat()), ["user~:hi"]);
        assert!(d.busy());
        assert!(!seen.iter().any(|e| matches!(e, Event::Delta(_))));

        d.send(Cmd::Played { reply: 1, ms: 100 });
        wait_for(&d, &mut seen, |e| e.contains(&Event::Delta("Hello".into())));
        assert_eq!(kinds(&store.chat()), ["user~:hi"]);
        d.send(Cmd::Played { reply: 1, ms: 200 });
        wait_for(&d, &mut seen, |e| e.contains(&Event::RunEnded));
        let chat = store.chat();
        assert_eq!(kinds(&chat), ["user~:hi", "assistant~:Hello there"]);
        assert!(matches!(&chat.entries[1].body, Body::Assistant { usage, model, .. }
            if usage.input == 7 && usage.output == 3 && model == "mock/gpt-realtime"));
        assert!(!d.busy());

        d.send(Cmd::Stop);
        wait_for(&d, &mut seen, |e| e.contains(&Event::VoiceEnded));
        let session = sent.try_iter().next().unwrap();
        assert_eq!(session["type"], "session.update");
        let instructions = session["session"]["instructions"].as_str().unwrap();
        assert!(instructions.contains("Lockbook") && instructions.ends_with(SPOKEN));
    }

    /// Speaking over a reply cuts it where it had played, the server is
    /// told the same, and the next turn follows it in the chat.
    #[test]
    fn speaking_over_a_reply_cuts_it_where_it_was_heard() {
        let mut steps = opened();
        steps.extend([committed("u1"), heard("u1", "tell me a story"), created()]);
        for i in 0..10 {
            steps.push(audio("a1", 100));
            steps.push(words("a1", &format!(" w{i}")));
        }
        steps.extend([
            Step::Expect("input_audio_buffer.append"),
            Step::Send(json!({ "type": "input_audio_buffer.speech_started" })),
            Step::Expect("conversation.item.truncate"),
            done("cancelled", vec![message("a1")], (9, 9)),
            committed("u2"),
            heard("u2", "stop"),
        ]);
        let (url, sent) = serve_ws(steps);
        let store = MemStore::default();
        let d = driver(store.clone(), NoTools, url);
        let mut seen = Vec::new();
        d.send(Cmd::StartVoice);
        wait_for(&d, &mut seen, |e| e.contains(&Event::VoiceStarted));
        d.send(chunk());
        wait_for(&d, &mut seen, |e| count(e, |e| matches!(e, Event::Audio { .. })) == 10);
        d.send(Cmd::Played { reply: 1, ms: 350 });
        d.send(chunk());
        wait_for(&d, &mut seen, |e| e.contains(&Event::Interrupted));
        wait_for(&d, &mut seen, |e| count(e, |e| matches!(e, Event::Written(_))) == 3);

        assert_eq!(
            kinds(&store.chat()),
            ["user~:tell me a story", "assistant~!:w0 w1 w2", "user~:stop"]
        );
        let told: Vec<&str> = seen
            .iter()
            .filter_map(|e| match e {
                Event::Delta(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(told, [" w0", " w1", " w2"]);
        assert!(seen.contains(&Event::RunEnded));
        let truncate = sent
            .try_iter()
            .find(|e| e["type"] == "conversation.item.truncate")
            .unwrap();
        assert_eq!((&truncate["item_id"], &truncate["audio_end_ms"]), (&json!("a1"), &json!(350)));
    }

    /// `echo` answers.
    struct Mock;

    impl Tools for Mock {
        fn schemas(&self) -> Vec<ToolSchema> {
            vec![ToolSchema {
                name: "echo".into(),
                description: "echo".into(),
                parameters: json!({}),
            }]
        }

        fn call(&mut self, call: &Call) -> ToolOutcome {
            ToolOutcome::ok(format!("echo:{}", call.args["text"].as_str().unwrap_or("")))
        }
    }

    /// A call is run as it lands, its answer posted, and a response asked
    /// for; the lines settle in the order of the conversation.
    #[test]
    fn a_call_is_run_and_answered_and_a_response_asked_for() {
        let call = json!({ "type": "function_call", "call_id": "c1", "name": "echo", "arguments": "{\"text\":\"x\"}" });
        let mut steps = opened();
        steps.extend([
            committed("u1"),
            heard("u1", "echo x"),
            created(),
            done("completed", vec![call], (1, 1)),
            Step::Expect("conversation.item.create"),
            Step::Expect("response.create"),
            created(),
            audio("a2", 100),
            words("a2", "echo said x"),
            done("completed", vec![message("a2")], (5, 2)),
        ]);
        let (url, sent) = serve_ws(steps);
        let store = MemStore::default();
        let d = driver(store.clone(), Mock, url);
        let mut seen = Vec::new();
        d.send(Cmd::StartVoice);
        wait_for(&d, &mut seen, |e| e.contains(&Event::VoiceStarted));
        d.send(chunk());
        wait_for(&d, &mut seen, |e| e.iter().any(|e| matches!(e, Event::Audio { reply: 1, .. })));
        d.send(Cmd::Played { reply: 1, ms: 100 });
        wait_for(&d, &mut seen, |e| e.contains(&Event::RunEnded));

        let chat = store.chat();
        assert_eq!(
            kinds(&chat),
            ["user~:echo x", "assistant~:", "tool:echo:echo:x", "assistant~:echo said x"]
        );
        assert!(
            matches!(&chat.entries[1].body, Body::Assistant { usage, .. } if *usage == Usage { input: 1, output: 1, ..Default::default() })
        );
        assert!(
            seen.iter()
                .any(|e| matches!(e, Event::ToolStarted(c) if c.name == "echo"))
        );
        let posted: Vec<Value> = sent.try_iter().collect();
        let output = posted
            .iter()
            .find(|e| e["item"]["type"] == "function_call_output")
            .unwrap();
        assert_eq!(
            (&output["item"]["call_id"], &output["item"]["output"]),
            (&json!("c1"), &json!("echo:x"))
        );
        let session = &posted[0]["session"];
        assert_eq!(session["tools"][0]["name"], "echo");
    }

    /// A session opened on a chat under way carries the chat in as items.
    #[test]
    fn a_session_opened_mid_chat_carries_the_chat() {
        let store = MemStore::default();
        store
            .update(&mut |chat| {
                chat.push(Entry::user("u", "look"));
                chat.push(Entry::assistant("u", "Sure.", "m", Usage::default()));
                chat.push(Entry::tool("u", "read", json!({ "path": "/a.md" }), "# A", true));
                chat.push(Entry::assistant("u", "It says A.", "m", Usage::default()));
            })
            .unwrap();
        let steps =
            vec![Step::Expect("session.update"), Step::Send(json!({ "type": "session.updated" }))];
        let (url, sent) = serve_ws(steps);
        let d = driver(store.clone(), NoTools, url);
        let mut seen = Vec::new();
        d.send(Cmd::StartVoice);
        wait_for(&d, &mut seen, |e| e.contains(&Event::VoiceStarted));
        d.send(Cmd::Stop);
        wait_for(&d, &mut seen, |e| e.contains(&Event::VoiceEnded));

        let posted: Vec<Value> = sent.iter().collect();
        let items: Vec<&Value> = posted
            .iter()
            .filter(|e| e["type"] == "conversation.item.create")
            .map(|e| &e["item"])
            .collect();
        assert_eq!(items.len(), 5, "{posted:?}");
        assert_eq!(items[0]["content"][0]["text"], "look");
        assert_eq!(items[1]["content"][0]["text"], "Sure.");
        assert_eq!(items[2]["name"], "read");
        assert_eq!(items[3]["output"], "# A");
        assert_eq!(items[4]["content"][0]["text"], "It says A.");
        assert_eq!(kinds(&store.chat()).len(), 4, "nothing was written");
    }

    /// The connection going settles what was heard of the reply, says
    /// what happened, and ends the run and the session.
    #[test]
    fn a_lost_connection_settles_what_was_heard() {
        let mut steps = opened();
        steps.extend([
            committed("u1"),
            heard("u1", "go on"),
            created(),
            audio("a1", 100),
            words("a1", "One"),
            audio("a1", 100),
            words("a1", " two"),
            audio("a1", 100),
            words("a1", " three"),
            Step::Expect("input_audio_buffer.append"),
            Step::Close,
        ]);
        let (url, _) = serve_ws(steps);
        let store = MemStore::default();
        let d = driver(store.clone(), NoTools, url);
        let mut seen = Vec::new();
        d.send(Cmd::StartVoice);
        wait_for(&d, &mut seen, |e| e.contains(&Event::VoiceStarted));
        d.send(chunk());
        wait_for(&d, &mut seen, |e| count(e, |e| matches!(e, Event::Audio { .. })) == 3);
        d.send(Cmd::Played { reply: 1, ms: 150 });
        d.send(chunk());
        wait_for(&d, &mut seen, |e| e.contains(&Event::VoiceEnded));

        assert_eq!(
            kinds(&store.chat()),
            ["user~:go on", "assistant~!:One", "error:voice: the connection closed"]
        );
        let ended = seen.iter().position(|e| *e == Event::RunEnded).unwrap();
        assert!(ended < seen.iter().position(|e| *e == Event::VoiceEnded).unwrap());
        assert!(!d.busy());
    }

    /// Stopping waits a moment for the words of the turn just spoken, and
    /// a reply nothing of which had played is not written as heard.
    #[test]
    fn stopping_keeps_what_was_heard_and_waits_for_the_last_words() {
        let mut steps = opened();
        steps.extend([
            committed("u1"),
            created(),
            words("a1", "Hi"),
            audio("a1", 100),
            Step::Wait(300),
            heard("u1", "late words"),
            done("completed", vec![message("a1")], (1, 1)),
        ]);
        let (url, _) = serve_ws(steps);
        let store = MemStore::default();
        let d = driver(store.clone(), NoTools, url);
        let mut seen = Vec::new();
        d.send(Cmd::StartVoice);
        wait_for(&d, &mut seen, |e| e.contains(&Event::VoiceStarted));
        d.send(chunk());
        wait_for(&d, &mut seen, |e| e.iter().any(|e| matches!(e, Event::Audio { .. })));
        d.send(Cmd::Stop);
        wait_for(&d, &mut seen, |e| e.contains(&Event::VoiceEnded));
        // The reply's words came ahead of any audio played, so none were heard.
        assert_eq!(kinds(&store.chat()), ["user~:late words"]);
    }

    /// Voice on a provider with nothing that speaks is one error line.
    #[test]
    fn a_provider_that_does_not_speak_says_so() {
        let config = Config {
            user: "u".into(),
            working_dir: "/".into(),
            provider: Box::new(|| {
                Ok(Provider {
                    name: "pop-os".into(),
                    display_name: None,
                    needs_key: false,
                    kind: Kind::OpenAi,
                    base_url: "http://pop-os:11435/v1".into(),
                    api_key: None,
                    model: "k2".into(),
                    effort: None,
                })
            }),
            window: |_| None,
        };
        let store = MemStore::default();
        let d = Driver::spawn(store.clone(), NoTools, config, || {});
        let mut seen = Vec::new();
        d.send(Cmd::StartVoice);
        wait_for(&d, &mut seen, |e| e.contains(&Event::VoiceEnded));
        assert_eq!(kinds(&store.chat()), ["error:Pop-os has no model that speaks"]);
    }
}
