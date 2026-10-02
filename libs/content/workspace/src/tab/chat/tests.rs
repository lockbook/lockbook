//! The chat tab driven headlessly, inside a real workspace: type, send,
//! watch the reply land, and watch the workspace save it. Needs a local
//! server (`lbdev ci start-server`), like the lb-rs integration tests.

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use egui::{Context, Event, Key, Modifiers, RawInput, Rect, pos2};
use lb_rs::Uuid;
use lb_rs::blocking::Lb;
use lb_rs::model::chat::{Body, Chat as Transcript, Entry, Usage};
use test_utils::{random_name, test_config, url};

use super::Chat;
use crate::file_cache::FileCache;
use crate::theme::palette_v2::{Mode, Theme, ThemeExt as _};
use crate::workspace::Workspace;

const SSE_HI: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
    data: {\"choices\":[{\"delta\":{\"content\":\"hi there\"}}]}\n\n\
    data: {\"choices\":[],\"usage\":{\"prompt_tokens\":4,\"completion_tokens\":2}}\n\n\
    data: [DONE]\n\n";

/// Serves `response` to every connection.
fn mock_provider(response: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        for sock in listener.incoming().flatten() {
            let mut sock = sock;
            let mut buf = Vec::new();
            let mut tmp = [0u8; 4096];
            loop {
                let n = sock.read(&mut tmp).unwrap_or(0);
                if n == 0 {
                    break;
                }
                buf.extend_from_slice(&tmp[..n]);
                let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") else { continue };
                let headers = String::from_utf8_lossy(&buf[..end]).to_lowercase();
                let len = headers
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .map_or(0, |v| v.trim().parse::<usize>().unwrap_or(0));
                if buf.len() >= end + 4 + len {
                    break;
                }
            }
            let _ = sock.write_all(response.as_bytes());
        }
    });
    format!("http://{addr}")
}

fn write(lb: &Lb, path: &str, text: &str) {
    let file = lb.create_at_path(path).unwrap();
    lb.write_document(file.id, text.as_bytes()).unwrap();
}

/// An account whose default provider is a mock that always says "hi there",
/// and an empty chat at `/home/c.chat`.
fn account_with_chat() -> (Lb, Uuid) {
    let lb = Lb::init(test_config()).unwrap();
    lb.create_account(&random_name(), &url(), false).unwrap();
    let provider = format!(
        "{{\"kind\":\"openai\",\"base_url\":\"{}\",\"model\":\"m\"}}",
        mock_provider(SSE_HI)
    );
    write(&lb, "/.agent/providers/mock.json", &provider);
    write(&lb, "/.agent/default.json", "{\"provider\":\"mock\"}");
    let file = lb.create_at_path("/home/c.chat").unwrap();
    (lb, file.id)
}

fn context() -> Context {
    let ctx = Context::default();
    let mut fonts = egui::FontDefinitions::default();
    crate::register_fonts(&mut fonts);
    ctx.set_fonts(fonts);
    ctx.set_lb_theme(Theme::default(Mode::Dark));
    crate::register_font_system(&ctx);
    ctx
}

fn raw_input(events: Vec<Event>) -> RawInput {
    RawInput {
        screen_rect: Some(Rect::from_min_max(pos2(0.0, 0.0), pos2(900.0, 700.0))),
        events,
        ..Default::default()
    }
}

fn key(key: Key) -> Event {
    Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: Modifiers::NONE }
}

/// (author, kind) of every visible line in the document.
fn lines(lb: &Lb, id: Uuid) -> Vec<(String, &'static str)> {
    Transcript::parse(&lb.read_document(id, false).unwrap())
        .entries
        .iter()
        .filter_map(|e| {
            let kind = match &e.body {
                Body::User { .. } => "user",
                Body::Assistant { .. } => "assistant",
                Body::Error { .. } => "error",
                _ => return None,
            };
            Some((e.from.clone(), kind))
        })
        .collect()
}

fn bob_says(lb: &Lb, id: Uuid, text: &str) -> Vec<u8> {
    let mut on_disk = Transcript::parse(&lb.read_document(id, false).unwrap());
    on_disk.push(Entry::user("bob", text));
    on_disk.push(Entry::assistant("bob", "bob's assistant replies", "m", Usage::default()));
    on_disk.serialize()
}

mod in_a_workspace {
    use super::*;

    fn frame(ctx: &Context, ws: &mut Workspace, events: Vec<Event>) {
        let _ = ctx.run(raw_input(events), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ws.show(ui);
            });
        });
    }

    fn frames_until(ctx: &Context, ws: &mut Workspace, done: impl Fn(&Workspace) -> bool) {
        let start = Instant::now();
        while !done(ws) {
            assert!(start.elapsed() < Duration::from_secs(20), "timed out");
            frame(ctx, ws, vec![]);
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// The current tab's chat once it has loaded.
    fn chat(ws: &Workspace) -> Option<&Chat> {
        ws.current_tab().and_then(|t| t.chat())
    }

    fn shows(ws: &Workspace, entries: usize) -> bool {
        chat(ws).is_some_and(|c| c.entry_count() == entries && !c.busy)
    }

    fn send(ctx: &Context, ws: &mut Workspace, text: &str) {
        frames_until(ctx, ws, |ws| chat(ws).is_some_and(|c| c.is_ready() && !c.busy));
        frame(ctx, ws, vec![Event::Text(text.into())]);
        frame(ctx, ws, vec![]);
        assert_eq!(chat(ws).unwrap().composer_text(), text);
        frame(ctx, ws, vec![key(Key::Enter)]);
        assert_eq!(chat(ws).unwrap().composer_text(), "", "the composer clears on send");
    }

    /// The driver appends in memory; the workspace's autosave notices and
    /// writes the document. A collaborator's lines written to the document
    /// behind the tab's back are merged in, shown, and kept by later saves,
    /// and only this user's messages get replies from this user's driver.
    #[test]
    fn the_workspace_saves_what_the_driver_appends() {
        let (lb, id) = account_with_chat();
        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let mut ws = Workspace::new(&lb, &ctx, true, false, Some(files));
        let me = lb.get_account().unwrap().username.clone();

        ws.open_file(id, true, false);
        send(&ctx, &mut ws, "hello");
        frames_until(&ctx, &mut ws, |ws| shows(ws, 2));
        frames_until(&ctx, &mut ws, |_| lines(&lb, id).len() == 2);
        assert_eq!(lines(&lb, id), [(me.clone(), "user"), (me.clone(), "assistant")]);

        lb.write_document(id, &bob_says(&lb, id, "hey from bob"))
            .unwrap();
        frames_until(&ctx, &mut ws, |ws| shows(ws, 4));

        send(&ctx, &mut ws, "and again");
        frames_until(&ctx, &mut ws, |ws| shows(ws, 6));
        frames_until(&ctx, &mut ws, |_| lines(&lb, id).len() == 6);
        assert_eq!(
            lines(&lb, id),
            [
                (me.clone(), "user"),
                (me.clone(), "assistant"),
                ("bob".into(), "user"),
                ("bob".into(), "assistant"),
                (me.clone(), "user"),
                (me.clone(), "assistant"),
            ]
        );
    }
}

mod on_its_own {
    use super::*;

    fn frame(ctx: &Context, chat: &mut Chat, events: Vec<Event>) {
        let _ = ctx.run(raw_input(events), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                chat.show(ui);
            });
        });
    }

    fn frames_until(ctx: &Context, chat: &mut Chat, done: impl Fn(&Chat) -> bool) {
        let start = Instant::now();
        while !done(chat) {
            assert!(start.elapsed() < Duration::from_secs(15), "timed out");
            frame(ctx, chat, vec![]);
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Every kind of row, from two people, in every live state, at a wide
    /// and a narrow window: the view lays all of it out without panicking.
    #[test]
    fn every_row_kind_lays_out() {
        let (lb, id) = account_with_chat();
        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let me = account.username.clone();

        let mut t = Transcript::default();
        t.push(Entry::user("bob", "hi from bob 🙂"));
        t.push(Entry::assistant("bob", "bob's reply", "m", Usage::default()));
        t.push(Entry::user(&me, "mine"));
        let tool = Entry::tool(
            &me,
            "edit",
            serde_json::json!({"path": "/home/a.md", "old": "x", "new": "y"}),
            "edited /home/a.md",
            true,
        );
        let tool_id = tool.id;
        t.push(tool);
        t.push(Entry::tool(&me, "search", serde_json::json!({"query": "q"}), "nothing", false));
        let mut reply = Entry::assistant(&me, "**bold** reply\n\n- a\n- b", "m", Usage::default());
        if let Body::Assistant { interrupted, .. } = &mut reply.body {
            *interrupted = true;
        }
        t.push(reply);
        t.push(Entry::error(&me, "rate limited"));
        let mut day_later = Entry::user(&me, "unanswered");
        day_later.ts += 86_400_000;
        t.push(day_later);
        let bytes = t.serialize();

        let mut chat = Chat::new(&bytes, id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut chat, |c| c.is_ready());
        assert_eq!(chat.entry_count(), 8);
        chat.expanded.insert(tool_id);
        for _ in 0..2 {
            frame(&ctx, &mut chat, vec![]);
        }

        chat.busy = true;
        chat.running_tool = Some(lb_chat::Call {
            id: "c".into(),
            name: "read".into(),
            args: serde_json::json!({"path": "/home/a.md", "section": "Plan"}),
        });
        frame(&ctx, &mut chat, vec![]);
        chat.running_tool = None;
        chat.streaming = "partial…".into();
        frame(&ctx, &mut chat, vec![]);
        chat.streaming.clear();
        chat.pending_ask = Some((
            lb_chat::Call { id: "c".into(), name: "delete".into(), args: serde_json::json!({}) },
            "Delete /home/a.md?".into(),
        ));
        frame(&ctx, &mut chat, vec![]);
        chat.pending_ask = None;
        frame(&ctx, &mut chat, vec![]);
        chat.busy = false;
        chat.adding_root = true;
        frame(&ctx, &mut chat, vec![]);

        let narrow = RawInput {
            screen_rect: Some(Rect::from_min_max(pos2(0.0, 0.0), pos2(300.0, 400.0))),
            ..Default::default()
        };
        let _ = ctx.run(narrow, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                chat.show(ui);
            });
        });
    }

    /// Lines the tab holds but has not saved survive a reload from disk,
    /// and the reload leaves the tab dirty so the merge gets written.
    #[test]
    fn unsaved_lines_survive_a_reload_from_disk() {
        let (lb, id) = account_with_chat();
        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let me = account.username.clone();
        let mut chat = Chat::new(b"", id, None, account, ctx.clone(), files, &lb);

        frames_until(&ctx, &mut chat, |c| c.is_ready());
        frame(&ctx, &mut chat, vec![Event::Text("hello".into())]);
        frame(&ctx, &mut chat, vec![key(Key::Enter)]);
        frames_until(&ctx, &mut chat, |c| c.entry_count() == 2 && !c.busy);
        assert!(chat.take_changed());
        assert!(lines(&lb, id).is_empty(), "nothing saves until the workspace does");

        let from_disk = bob_says(&lb, id, "hey from bob");
        chat.reload(&from_disk, None);
        frame(&ctx, &mut chat, vec![]);
        assert_eq!(chat.entry_count(), 4);
        assert!(chat.take_changed(), "the merge holds lines disk does not");

        let merged = chat.to_bytes();
        let authors: Vec<String> = Transcript::parse(&merged)
            .entries
            .iter()
            .map(|e| e.from.clone())
            .collect();
        assert_eq!(authors, [me.clone(), me.clone(), "bob".into(), "bob".into()]);

        let hmac = lb.safe_write(id, None, merged.clone(), None).unwrap();
        chat.saved(hmac, merged.clone());
        chat.reload(&merged, Some(hmac));
        assert!(!chat.take_changed(), "disk and tab agree");
    }
}
