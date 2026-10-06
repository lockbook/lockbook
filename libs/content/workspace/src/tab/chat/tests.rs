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

use super::model_sheet::Item;
use super::setup::{OWN, Offered, TEMPLATES};
use super::{Chat, ListingState};
use crate::file_cache::FileCache;
use crate::theme::palette_v2::{Mode, Theme, ThemeExt as _};
use crate::workspace::Workspace;

const SSE_HI: &str = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
    data: {\"choices\":[{\"delta\":{\"content\":\"hi there\"}}]}\n\n\
    data: {\"choices\":[],\"usage\":{\"prompt_tokens\":4,\"completion_tokens\":2}}\n\n\
    data: [DONE]\n\n";

/// A model server's listing: two models, the second the newer.
const MODELS: &str = "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n\
    {\"data\":[{\"id\":\"older\",\"created\":1},{\"id\":\"newer\",\"created\":2}]}";

/// Serves `response` to every connection.
fn mock_provider(response: &'static str) -> String {
    mock_server(response, response)
}

/// Serves `listing` to every GET and `reply` to everything else.
fn mock_server(listing: &'static str, reply: &'static str) -> String {
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
            let response = if buf.starts_with(b"GET") { listing } else { reply };
            let _ = sock.write_all(response.as_bytes());
        }
    });
    format!("http://{addr}")
}

/// A provider file as the tab holds it.
fn offered(name: &str, base_url: &str) -> Offered {
    let file = serde_json::json!({ "base_url": base_url }).to_string();
    Offered { name: name.into(), file: lb_chat::Provider::parse(name, "", file.as_bytes()).ok() }
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

pub(super) fn context() -> Context {
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

    fn frame(ctx: &Context, ws: &mut Workspace, events: Vec<Event>) -> crate::output::Response {
        let mut out = Default::default();
        let _ = ctx.run(raw_input(events), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                out = ws.show(ui);
            });
        });
        out
    }

    /// A click at `pos`; what the workspace reported over its frames.
    fn click(ctx: &Context, ws: &mut Workspace, pos: egui::Pos2) -> Vec<crate::output::Response> {
        let button = |pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        [vec![Event::PointerMoved(pos)], vec![button(true)], vec![button(false)], vec![]]
            .into_iter()
            .map(|events| frame(ctx, ws, events))
            .collect()
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

    /// A note a message links to goes with it: it is read as the message
    /// is sent, once however often it is linked, and a link out of the
    /// vault attaches nothing.
    #[test]
    fn a_linked_note_goes_with_the_message() {
        let (lb, id) = account_with_chat();
        write(&lb, "/home/plan.md", "ship in october");
        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let mut ws = Workspace::new(&lb, &ctx, true, false, Some(files));
        ws.open_file(id, true, false);
        send(&ctx, &mut ws, "see [plan](plan.md), [again](/home/plan.md), [web](https://x.test)");
        frames_until(&ctx, &mut ws, |ws| shows(ws, 3));

        let entries = &chat(&ws).unwrap().transcript.entries;
        let Body::User { mentions, .. } = &entries[0].body else { panic!() };
        let paths: Vec<&str> = mentions.iter().map(|m| m.path.as_str()).collect();
        assert_eq!(paths, ["/home/plan.md"]);
        assert!(
            matches!(&entries[1].body, Body::Tool { name, result, .. } if name == "read" && result == "ship in october")
        );
    }

    /// A picture pasted into the composer becomes a file beside the chat
    /// and a link in the draft, and so goes with the message when sent.
    #[test]
    fn a_pasted_picture_is_imported_and_goes_with_the_message() {
        use crate::tab::{ClipContent, Event as TabEvent, ExtendedInput as _};
        let (lb, id) = account_with_chat();
        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let mut ws = Workspace::new(&lb, &ctx, true, false, Some(files));
        ws.open_file(id, true, false);
        frames_until(&ctx, &mut ws, |ws| chat(ws).is_some_and(|c| c.is_ready()));

        let mut png = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(8, 8)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let content = vec![ClipContent::Image(png.into_inner())];
        ctx.push_event(TabEvent::Paste { content, position: pos2(0.0, 0.0) });
        frame(&ctx, &mut ws, vec![]);
        frame(&ctx, &mut ws, vec![]);
        let draft = chat(&ws).unwrap().composer_text();
        assert!(draft.starts_with("![") && draft.contains("imports/"), "{draft}");

        frame(&ctx, &mut ws, vec![key(Key::Enter)]);
        frames_until(&ctx, &mut ws, |ws| shows(ws, 3));
        let entries = &chat(&ws).unwrap().transcript.entries;
        let Body::User { mentions, .. } = &entries[0].body else { panic!() };
        assert!(
            mentions.len() == 1 && mentions[0].path.starts_with("/home/imports/"),
            "{mentions:?}"
        );
        assert!(
            matches!(&entries[1].body, Body::Tool { name, result, .. } if name == "read" && result.contains("is a picture, 8 by 8"))
        );
    }

    /// A folder among a card's files is somewhere to go, not something to
    /// open: a click makes it the workspace's folder, which is what each
    /// client's file tree follows. The chat stays where it is.
    #[test]
    fn a_folder_in_a_card_takes_the_file_tree_there() {
        let (lb, id) = account_with_chat();
        write(&lb, "/home/notes/plan.md", "x");
        let notes = lb.get_by_path("/home/notes").unwrap().id;
        let me = lb.get_account().unwrap().username.clone();
        let mut t = Transcript::default();
        t.push(Entry::user(&me, "what is here"));
        let list = serde_json::json!({"path": "/home/"});
        let list = Entry::tool(&me, "list", list, "notes/\nc.chat", true);
        let card = list.id;
        t.push(list);
        t.push(Entry::assistant(&me, "that is all", "m", Usage::default()));
        lb.write_document(id, &t.serialize()).unwrap();

        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let mut ws = Workspace::new(&lb, &ctx, true, false, Some(files));
        ws.open_file(id, true, false);
        frames_until(&ctx, &mut ws, |ws| shows(ws, 3));
        for _ in 0..3 {
            frame(&ctx, &mut ws, vec![]);
        }
        let rect = |id: egui::Id| ctx.read_response(id).map(|r| r.rect);
        let bar = rect(egui::Id::new(("chat_tool", card))).expect("the call's bar");
        click(&ctx, &mut ws, bar.center());
        let folder = rect(egui::Id::new(("chat_tool_file", (card, 0usize)))).expect("the folder");

        let reported = click(&ctx, &mut ws, folder.center());
        assert!(
            reported
                .iter()
                .any(|out| out.selected_file == Some(notes) && out.selected_folder_changed),
            "the clients are told"
        );
        assert_eq!(ws.focused_parent, Some(notes));
        assert_eq!(ws.current_tab().and_then(|t| t.id()), Some(id), "the chat is still the tab");
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
        // One call of each kind of card: a diff, a reason, files with and
        // without snippets, a note, plain text, and a bare confirmation
        // whose path is longer than any window.
        let json = |text: &str| serde_json::from_str::<serde_json::Value>(text).unwrap();
        let long = format!("/home/{}.md", "long-name-".repeat(40));
        let calls = [
            (
                "edit",
                json(r#"{"path": "/home/a.md", "old": "one two", "new": "one 2"}"#),
                "ok",
                true,
            ),
            ("edit", json(r#"{"path": "/home/a.md", "old": "x", "new": "y"}"#), "not found", false),
            (
                "search",
                json(r#"{"query": "q"}"#),
                "/home/c.chat\n  a q here\n/home/x.md\n(2 more)",
                true,
            ),
            ("list", json(r#"{}"#), "notes/\nc.chat", true),
            ("read", json(r#"{"path": "/home/a.md"}"#), "# Title\n\n- a\n- b", true),
            ("read", json(r#"{"path": "/home/a.txt"}"#), "plain\ntext", true),
            ("delete", serde_json::json!({ "path": long }), "deleted", true),
        ];
        let mut tool_ids = Vec::new();
        for (name, args, result, ok) in calls {
            let tool = Entry::tool(&me, name, args, result, ok);
            tool_ids.push(tool.id);
            t.push(tool);
        }
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
        assert_eq!(chat.entry_count(), 13);
        // Closed, then every card open.
        frame(&ctx, &mut chat, vec![]);
        chat.expanded.extend(tool_ids);
        for _ in 0..2 {
            frame(&ctx, &mut chat, vec![]);
        }

        chat.busy = true;
        chat.running_tool = Some(lb_chat::Call {
            id: "c".into(),
            name: "read".into(),
            args: serde_json::json!({"path": "/home/a.md", "section": "Plan"}),
            echo: None,
        });
        frame(&ctx, &mut chat, vec![]);
        chat.running_tool = None;
        chat.streaming = "partial…".into();
        frame(&ctx, &mut chat, vec![]);
        chat.streaming.clear();
        chat.busy = false;
        chat.scope_open = true;
        frame(&ctx, &mut chat, vec![]);
        chat.scope_open = false;
        // The model sheet: two providers' listings, long enough to scroll,
        // drawn open, scrolled so a provider pins, and with one folded.
        let listing = |n: usize| -> Vec<lb_chat::ModelInfo> {
            (0..n)
                .map(|i| lb_chat::ModelInfo {
                    id: format!("model-{i}"),
                    display_name: None,
                    window: None,
                })
                .collect()
        };
        // A brand, this device, a machine on the network, a file that does
        // not parse.
        chat.providers = vec![
            offered("anthropic", "https://api.anthropic.com/v1"),
            offered("mock", "http://localhost:11434/v1"),
            offered("other", "http://linux-box:11434/v1"),
            Offered { name: "broken".into(), file: None },
        ];
        chat.listings
            .insert("mock".into(), ListingState::Ready(listing(30)));
        chat.listings
            .insert("other".into(), ListingState::Failed("offline".into()));
        chat.favorites = vec!["mock/model-3".into()];
        chat.models_open = true;
        chat.model_dest = Some("mock/model-20".into());
        chat.model_scroll_top = true;
        for _ in 0..3 {
            frame(&ctx, &mut chat, vec![]);
        }
        chat.model_reveal =
            Some(super::super::model_sheet::Reveal::Hold { row: "mock".into(), vy: 0.0 });
        chat.model_folded.insert("mock".into());
        frame(&ctx, &mut chat, vec![]);
        chat.model_filter = "model-2".into();
        frame(&ctx, &mut chat, vec![]);
        chat.model_filter = "nothing matches".into();
        frame(&ctx, &mut chat, vec![]);
        chat.models_open = false;

        let narrow = |chat: &mut Chat| {
            let narrow = RawInput {
                screen_rect: Some(Rect::from_min_max(pos2(0.0, 0.0), pos2(300.0, 400.0))),
                ..Default::default()
            };
            let _ = ctx.run(narrow, |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    chat.show(ui);
                });
            });
        };
        narrow(&mut chat);

        // The setup form: nothing picked, a hosted provider, a server of the
        // user's own, its error, and the wait while it is asked for models.
        chat.begin_connect(None);
        frame(&ctx, &mut chat, vec![]);
        chat.setup.pick(&TEMPLATES[0]);
        frame(&ctx, &mut chat, vec![]);
        chat.setup.pick(&OWN);
        frame(&ctx, &mut chat, vec![]);
        chat.setup.error = Some("can't reach localhost:11434".into());
        frame(&ctx, &mut chat, vec![]);
        let (_pending, answer) = std::sync::mpsc::channel();
        chat.setup.asking = Some(answer);
        frame(&ctx, &mut chat, vec![]);
        narrow(&mut chat);
    }

    fn click(ctx: &Context, chat: &mut Chat, pos: egui::Pos2) {
        let button = |pressed| Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        frame(ctx, chat, vec![Event::PointerMoved(pos)]);
        frame(ctx, chat, vec![button(true)]);
        frame(ctx, chat, vec![button(false)]);
        frame(ctx, chat, vec![]);
    }

    /// Where `item`'s row took the pointer last frame: its pinned copy when
    /// it is stuck to the top of the list, else the row in the flow.
    fn model_row(ctx: &Context, chat: &Chat, item: &Item) -> Rect {
        let row = |pinned: bool| {
            let id = egui::Id::new(("chat_model_row", chat.id)).with((item.flat().id, pinned));
            ctx.read_response(id).map(|r| r.rect)
        };
        [row(true), row(false)]
            .into_iter()
            .flatten()
            .max_by(|a, b| a.height().total_cmp(&b.height()))
            .expect("the row was drawn")
    }

    /// The model sheet by pointer and keyboard: typing filters, a provider
    /// row folds and unfolds, a model row is chosen, its pin lands in the
    /// vault, and Enter makes the choice this chat's model.
    #[test]
    fn the_model_sheet_folds_pins_and_chooses() {
        let (lb, id) = account_with_chat();
        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let mut chat = Chat::new(b"", id, None, account, ctx.clone(), files, &lb);
        // The mock provider answers its own listing request with nonsense;
        // let that land, then stand a real listing in its place.
        frames_until(&ctx, &mut chat, |c| {
            matches!(c.listings.get("mock"), Some(ListingState::Failed(_)))
        });
        let models = ["model-0", "model-1", "model-2"].map(|id| lb_chat::ModelInfo {
            id: id.into(),
            display_name: None,
            window: None,
        });
        chat.listings
            .insert("mock".into(), ListingState::Ready(models.to_vec()));
        chat.open_model_sheet();
        // The sheet sizes itself over its first frames.
        for _ in 0..6 {
            frame(&ctx, &mut chat, vec![]);
        }

        // The sheet owns the keyboard: typing filters instead of reaching the
        // composer behind it, and Esc clears the filter before it closes.
        frame(&ctx, &mut chat, vec![Event::Text("2".into())]);
        frame(&ctx, &mut chat, vec![]);
        assert_eq!((chat.model_filter.as_str(), chat.composer_text().as_str()), ("2", ""));
        frame(&ctx, &mut chat, vec![key(Key::Escape)]);
        assert!(chat.model_filter.is_empty() && chat.models_open);
        // A response read between frames can be a frame stale: let the
        // unfiltered list settle.
        frame(&ctx, &mut chat, vec![]);
        frame(&ctx, &mut chat, vec![]);

        let provider = Item::Provider { name: "mock".into(), open: true };
        let model = Item::Model { selection: "mock/model-1".into(), label: String::new() };
        let header = model_row(&ctx, &chat, &provider).center();
        click(&ctx, &mut chat, header);
        assert!(chat.model_folded.contains("mock"), "a provider row folds");
        click(&ctx, &mut chat, header);
        assert!(!chat.model_folded.contains("mock"), "and unfolds");

        let row = model_row(&ctx, &chat, &model);
        click(&ctx, &mut chat, row.center());
        assert_eq!(chat.model_dest.as_deref(), Some("mock/model-1"));

        let pin = pos2(row.right() - 12.0, row.center().y);
        click(&ctx, &mut chat, pin);
        assert_eq!(chat.favorites, ["mock/model-1"]);
        let saved = lb.get_by_path("/.agent/favorites.json").unwrap();
        let saved: Vec<String> =
            serde_json::from_slice(&lb.read_document(saved.id, false).unwrap()).unwrap();
        assert_eq!(saved, ["mock/model-1"]);
        assert_eq!(chat.model_dest.as_deref(), Some("mock/model-1"), "the pin is not the row");

        frame(&ctx, &mut chat, vec![key(Key::Enter)]);
        assert!(!chat.models_open);
        assert_eq!(
            chat.settings().model.as_deref(),
            Some("mock/model-1"),
            "settings change at once"
        );
        frames_until(
            &ctx,
            &mut chat,
            |c| matches!(&c.provider, Some(Ok(p)) if p.model == "model-1"),
        );
        assert_eq!(chat.settings().model.as_deref(), Some("mock/model-1"));

        // The pick is sticky: a chat with no choice of its own starts there.
        let other = lb.create_at_path("/home/next.chat").unwrap();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let mut next = Chat::new(b"", other.id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut next, |c| c.is_ready());
        assert!(next.settings().model.is_none());
        assert!(matches!(&next.provider, Some(Ok(p)) if p.selection() == "mock/model-1"));
    }

    /// A provider file that still holds a template's placeholder key. The
    /// model sheet offers to take the key instead of listing; choosing the
    /// provider brings up its form rather than a composer; connecting keeps
    /// the vault's default and points this chat at the provider.
    #[test]
    fn a_keyless_provider_asks_for_its_key() {
        let (lb, id) = account_with_chat();
        let read = |path: &str| -> serde_json::Value {
            let file = lb.get_by_path(path).unwrap();
            serde_json::from_slice(&lb.read_document(file.id, false).unwrap()).unwrap()
        };
        let endpoint = read("/.agent/providers/mock.json")["base_url"]
            .as_str()
            .unwrap()
            .to_string();
        let groq = serde_json::json!({
            "display_name": "Groq",
            "base_url": endpoint,
            "model": "g",
            "api_key": "YOUR API KEY HERE",
        });
        write(&lb, "/.agent/providers/groq.json", &groq.to_string());

        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let mut chat = Chat::new(b"", id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut chat, |c| c.is_ready());

        // The sheet: no request goes out; the row opens the form; Esc returns.
        chat.open_model_sheet();
        frames_until(&ctx, &mut chat, |c| {
            matches!(c.listings.get("groq"), Some(ListingState::NeedsKey))
        });
        for _ in 0..6 {
            frame(&ctx, &mut chat, vec![]);
        }
        let row = model_row(&ctx, &chat, &Item::Connect { provider: "groq".into() });
        click(&ctx, &mut chat, row.center());
        assert!(!chat.models_open && !chat.is_ready());
        assert_eq!(chat.setup.picked.map(|t| t.name), Some("groq"));
        assert_eq!(chat.setup.base_url, endpoint, "the form keeps what the file says");
        frame(&ctx, &mut chat, vec![key(Key::Escape)]);
        assert!(chat.is_ready(), "Esc goes back to the provider that works");

        // Choosing the keyless provider: the form again, this time to stay.
        let mut settings = chat.settings();
        settings.model = Some("groq".into());
        chat.set_settings(settings);
        frames_until(&ctx, &mut chat, |c| matches!(&c.provider, Some(Ok(p)) if p.name == "groq"));
        assert!(!chat.is_ready());
        assert_eq!(chat.setup.picked.map(|t| t.name), Some("groq"));
        chat.setup.key = "gsk-test".into();
        chat.connect();
        frames_until(&ctx, &mut chat, |c| c.is_ready());

        assert_eq!(read("/.agent/providers/groq.json")["api_key"], "gsk-test");
        let default = read("/.agent/default.json");
        assert_eq!(
            (default["provider"].as_str(), default["model"].as_str()),
            (Some("groq"), Some("g")),
            "the last provider connected is what a new chat starts with"
        );
        assert_eq!(chat.settings().model.as_deref(), Some("groq/g"));
        assert!(matches!(&chat.provider, Some(Ok(p)) if p.name == "groq" && !p.needs_key));
    }

    /// A server of the user's own, by its address alone. Nothing listening:
    /// the form says so and writes nothing. A server that answers and no
    /// model named: its newest is chosen, the file is named for the host,
    /// and messages go to it.
    #[test]
    fn a_server_of_your_own_connects_by_its_address() {
        let (lb, id) = account_with_chat();
        let read = |path: &str| -> serde_json::Value {
            let file = lb.get_by_path(path).unwrap();
            serde_json::from_slice(&lb.read_document(file.id, false).unwrap()).unwrap()
        };
        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let mut chat = Chat::new(b"", id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut chat, |c| c.is_ready());

        chat.begin_connect(None);
        chat.setup.pick(&OWN);
        let closed = TcpListener::bind("127.0.0.1:0").unwrap();
        let dead = closed.local_addr().unwrap();
        drop(closed);
        chat.setup.base_url = dead.to_string();
        chat.connect();
        assert!(chat.setup.asking.is_some(), "no model named, so the server is asked");
        frames_until(&ctx, &mut chat, |c| c.setup.asking.is_none());
        assert_eq!(chat.setup.error, Some(format!("can't reach {dead}")));
        assert!(lb.get_by_path("/.agent/providers/127.0.0.1.json").is_err());

        // Typed the way people say an address: no scheme, no path.
        let server = mock_server(MODELS, SSE_HI);
        chat.setup.base_url = server.trim_start_matches("http://").to_string();
        chat.connect();
        frames_until(&ctx, &mut chat, |c| {
            c.is_ready() && matches!(&c.provider, Some(Ok(p)) if p.name == "127.0.0.1")
        });

        let file = read("/.agent/providers/127.0.0.1.json");
        assert_eq!(file["base_url"], format!("{server}/v1"));
        assert_eq!(
            (file["display_name"].as_str(), file["model"].as_str()),
            (Some("This device"), Some("newer"))
        );
        assert!(file.get("api_key").is_none(), "no key was given, so none is written");
        assert_eq!(chat.settings().model.as_deref(), Some("127.0.0.1/newer"));
        assert_eq!(read("/.agent/default.json")["provider"], "127.0.0.1");

        frame(&ctx, &mut chat, vec![Event::Text("hello".into())]);
        frame(&ctx, &mut chat, vec![key(Key::Enter)]);
        frames_until(&ctx, &mut chat, |c| c.entry_count() == 2 && !c.busy);
        let reply = chat.transcript.entries.last().unwrap();
        assert!(matches!(&reply.body, Body::Assistant { model, .. } if model == "127.0.0.1/newer"));
        assert_eq!(reply.text(), "hi there");
    }

    /// A provider file no template made, still holding a placeholder key:
    /// the form opens on it as a server of the user's own and rewrites that
    /// file, with the model as typed and nothing asked of the server.
    #[test]
    fn a_file_with_no_template_is_rewritten_in_place() {
        let (lb, id) = account_with_chat();
        let read = |path: &str| -> serde_json::Value {
            let file = lb.get_by_path(path).unwrap();
            serde_json::from_slice(&lb.read_document(file.id, false).unwrap()).unwrap()
        };
        let legacy = serde_json::json!({
            "base_url": "https://api.example.com/v1",
            "model": "model-id",
            "api_key": "YOUR API KEY HERE",
        });
        write(&lb, "/.agent/providers/custom.json", &legacy.to_string());
        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let mut chat = Chat::new(b"", id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut chat, |c| c.is_ready());

        chat.begin_connect(Some("custom"));
        assert!(chat.setup.own());
        assert_eq!(chat.setup.name.as_deref(), Some("custom"));
        assert_eq!(
            (chat.setup.base_url.as_str(), chat.setup.model.as_str()),
            ("https://api.example.com/v1", "model-id"),
            "the form keeps what the file says"
        );
        let server = mock_provider(SSE_HI);
        chat.setup.base_url = server.clone();
        chat.setup.model = "typed".into();
        chat.connect();
        assert!(chat.setup.asking.is_none(), "a model was named, so nothing is asked");
        frames_until(&ctx, &mut chat, |c| {
            c.is_ready() && matches!(&c.provider, Some(Ok(p)) if p.name == "custom")
        });

        let file = read("/.agent/providers/custom.json");
        assert_eq!(file["base_url"], format!("{server}/v1"));
        assert_eq!(file["model"], "typed");
        assert!(file.get("api_key").is_none(), "the placeholder is gone");
        assert!(lb.get_by_path("/.agent/providers/127.0.0.1.json").is_err());
    }

    /// Long messages wrap inside their bubbles and the reply starts below
    /// them; the bubble painter asserts its text fits.
    #[test]
    fn long_messages_fit_their_bubbles() {
        let (lb, id) = account_with_chat();
        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let me = account.username.clone();
        let long: String = (0..120).map(|i| format!("word{i} ")).collect();
        let mut t = Transcript::default();
        t.push(Entry::user(&me, long.trim()));
        t.push(Entry::assistant(&me, long.repeat(2), "m", Usage::default()));
        t.push(Entry::user(&me, "short"));
        let mut chat = Chat::new(&t.serialize(), id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut chat, |c| c.is_ready());
        for _ in 0..3 {
            frame(&ctx, &mut chat, vec![]);
        }
        let narrow = RawInput {
            screen_rect: Some(Rect::from_min_max(pos2(0.0, 0.0), pos2(420.0, 600.0))),
            ..Default::default()
        };
        for _ in 0..2 {
            let _ = ctx.run(narrow.clone(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    chat.show(ui);
                });
            });
        }
    }

    /// A settled call opens on a click into a card, and a file inside it
    /// opens in the workspace on another.
    #[test]
    fn a_card_opens_and_its_files_open() {
        let (lb, id) = account_with_chat();
        write(&lb, "/home/plan.md", "we ship chat in october");
        let plan = lb.get_by_path("/home/plan.md").unwrap().id;
        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let me = account.username.clone();
        let mut t = Transcript::default();
        t.push(Entry::user(&me, "find it"));
        let search = Entry::tool(
            &me,
            "search",
            serde_json::json!({"query": "chat"}),
            "/home/plan.md\n  we ship chat in october\n/home/gone.md",
            true,
        );
        let card = search.id;
        t.push(search);
        t.push(Entry::assistant(&me, "found it", "m", Usage::default()));
        let mut chat = Chat::new(&t.serialize(), id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut chat, |c| c.is_ready());
        for _ in 0..3 {
            frame(&ctx, &mut chat, vec![]);
        }
        let rect = |id: egui::Id| ctx.read_response(id).map(|r| r.rect);
        let file = |i: usize| egui::Id::new(("chat_tool_file", (card, i)));
        assert!(rect(file(0)).is_none(), "a card starts closed");

        let bar = rect(egui::Id::new(("chat_tool", card))).expect("the call's bar");
        click(&ctx, &mut chat, bar.center());
        assert!(chat.expanded.contains(&card));
        use crate::tab::ExtendedOutput as _;
        let _ = ctx.pop_open_files();
        click(&ctx, &mut chat, rect(file(0)).expect("the first hit").center());
        assert_eq!(ctx.pop_open_files(), [(plan, false)]);
        // A note that is no longer there has nothing to open.
        click(&ctx, &mut chat, rect(file(1)).expect("the second hit").center());
        assert_eq!(ctx.pop_open_files(), []);

        click(&ctx, &mut chat, bar.center());
        assert!(!chat.expanded.contains(&card), "a second click closes it");
        frame(&ctx, &mut chat, vec![]);
        assert!(rect(file(0)).is_none() && chat.bodies.is_empty());
    }

    /// A reply leads with a row for what it showed of its thinking, and one
    /// that only thought before calling a tool is still that row.
    #[test]
    fn a_reply_leads_with_what_it_thought() {
        let (lb, id) = account_with_chat();
        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let me = account.username.clone();
        let thinking = |text: &str, thought: &str| {
            let mut reply = Entry::assistant(&me, text, "m", Usage::default());
            if let Body::Assistant { thinking, .. } = &mut reply.body {
                *thinking = thought.into();
            }
            reply
        };
        let mut t = Transcript::default();
        t.push(Entry::user(&me, "when does it arrive"));
        let silent = thinking("", "the timetable will say");
        let spoken = thinking("12:15", "9:40 and 2:35");
        let (silent_id, spoken_id) = (silent.id, spoken.id);
        t.push(silent);
        t.push(Entry::tool(&me, "read", serde_json::json!({"path": "/t.md"}), "9:40", true));
        t.push(spoken);
        let mut chat = Chat::new(&t.serialize(), id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut chat, |c| c.is_ready());
        for _ in 0..3 {
            frame(&ctx, &mut chat, vec![]);
        }
        let rect = |id: Uuid| {
            ctx.read_response(egui::Id::new(("chat_tool", id)))
                .map(|r| r.rect)
        };
        let silent_row = rect(silent_id).expect("the row of a reply that only thought");
        let spoken_row = rect(spoken_id).expect("the row above a reply's text");
        assert!(silent_row.bottom() < spoken_row.top());

        click(&ctx, &mut chat, spoken_row.center());
        frame(&ctx, &mut chat, vec![]);
        let note = crate::tab::chat::rows::Part::Note("9:40 and 2:35".into());
        assert_eq!(chat.bodies.get(&spoken_id), Some(&vec![note]));
    }

    /// A thought can be read while it is still arriving, and stays open on
    /// the reply it settles into.
    #[test]
    fn a_thought_opens_while_it_arrives() {
        use super::super::IN_FLIGHT;
        use lb_chat::Event as Heard;
        let (lb, id) = account_with_chat();
        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let me = account.username.clone();
        let mut t = Transcript::default();
        t.push(Entry::user(&me, "when does it arrive"));
        let mut chat = Chat::new(&t.serialize(), id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut chat, |c| c.is_ready());
        chat.hear(Heard::RunStarted);
        chat.hear(Heard::Thinking("9:40 and".into()));
        for _ in 0..3 {
            frame(&ctx, &mut chat, vec![]);
        }
        let row = ctx
            .read_response(egui::Id::new(("chat_tool", IN_FLIGHT)))
            .expect("the row of the thought in flight");
        click(&ctx, &mut chat, row.rect.center());
        assert!(chat.expanded.contains(&IN_FLIGHT));
        // What has arrived is laid out afresh, so more of it shows.
        chat.hear(Heard::Thinking(" 2:35".into()));
        frame(&ctx, &mut chat, vec![]);
        assert!(!chat.bodies.contains_key(&IN_FLIGHT));

        let reply = Entry::assistant(&me, "12:15", "m", Usage::default());
        chat.hear(Heard::Written(reply.clone()));
        assert!(chat.expanded.contains(&reply.id) && !chat.expanded.contains(&IN_FLIGHT));
        assert!(chat.thinking.is_empty());
    }

    /// A note quoted in a card keeps its own bearings: a relative link in
    /// it leads where it does in the note. A right click on the row offers
    /// ways to the note itself.
    #[test]
    fn a_quoted_note_links_from_where_it_lives() {
        use crate::resolvers::link::ResolvedLink;
        let (lb, id) = account_with_chat();
        write(&lb, "/home/trips/plan.md", "see [packing](packing.md)");
        write(&lb, "/home/trips/packing.md", "socks");
        let packing = lb.get_by_path("/home/trips/packing.md").unwrap().id;
        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let me = account.username.clone();
        let mut t = Transcript::default();
        t.push(Entry::user(&me, "read the plan"));
        let args = serde_json::json!({"path": "/home/trips/plan.md"});
        let read = Entry::tool(&me, "read", args, "see [packing](packing.md)", true);
        let card = read.id;
        t.push(read);
        let mut chat = Chat::new(&t.serialize(), id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut chat, |c| c.is_ready());
        for _ in 0..3 {
            frame(&ctx, &mut chat, vec![]);
        }
        let row = egui::Id::new(("chat_tool", card));
        let bar = ctx.read_response(row).expect("the call's bar").rect;
        click(&ctx, &mut chat, bar.center());
        frame(&ctx, &mut chat, vec![]);
        let resolver = &chat.labels[&card].renderer.link_resolver;
        assert!(
            matches!(resolver.resolve_link("packing.md"), Some(ResolvedLink::File(f)) if f == packing)
        );

        let button = |pressed| Event::PointerButton {
            pos: bar.center(),
            button: egui::PointerButton::Secondary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        frame(&ctx, &mut chat, vec![button(true)]);
        frame(&ctx, &mut chat, vec![button(false)]);
        assert!(crate::style::context_menu::is_open_id(&ctx, row));
    }

    /// The wheel over a reply scrolls the chat: a reply whose rect was
    /// rounded a hair short of its text does not keep the wheel for itself.
    #[test]
    fn a_reply_leaves_the_wheel_to_the_chat() {
        let ctx = context();
        let (mut chat, _) = long_chat(&ctx);
        let reply = chat
            .transcript
            .entries
            .iter()
            .find(|e| matches!(e.body, Body::Assistant { .. }));
        let id = reply.unwrap().id;
        let text = reply.unwrap().text().to_string();
        let rect = Rect::from_min_size(pos2(40.0, 40.0), egui::vec2(400.0, 10.0));
        let mut left = 0.0;
        let _ = ctx.run(
            raw_input(vec![
                Event::PointerMoved(rect.center()),
                Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta: egui::vec2(0.0, -30.0),
                    modifiers: Modifiers::NONE,
                },
            ]),
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    chat.reader(id, &text)
                        .show(ui, rect, egui::Id::new("short"));
                    left = ui.input(|i| i.smooth_scroll_delta.y);
                });
            },
        );
        assert!(left != 0.0, "the reader took the wheel");
    }

    /// A chat opened again is where it was left: the same entry at the top
    /// of the view. One left following its end keeps no place.
    #[test]
    fn a_chat_opens_where_it_was_left() {
        use crate::workspace::WsPersistentStore;
        let ctx = context();
        let (mut chat, _) = long_chat(&ctx);
        let store = WsPersistentStore::new(false, std::env::temp_dir().join("unused.json"), false);
        chat.persistence = Some(store.clone());
        let rest = |ctx: &Context, chat: &mut Chat| (0..20).for_each(|_| frame(ctx, chat, vec![]));
        rest(&ctx, &mut chat);
        assert!(!store.data.read().unwrap().chat.contains_key(&chat.id));

        let wheel = Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 300.0),
            modifiers: Modifiers::NONE,
        };
        frame(&ctx, &mut chat, vec![Event::PointerMoved(pos2(450.0, 300.0)), wheel]);
        rest(&ctx, &mut chat);
        let (entry, _) = store.data.read().unwrap().chat[&chat.id];
        let top_of = |chat: &Chat| chat.spans.iter().find(|s| s.0 == entry).unwrap().1;
        let left_at = top_of(&chat);

        let again = context();
        let (bytes, account) = (chat.transcript.serialize(), chat.account.clone());
        let files = Arc::clone(&chat.files);
        let mut back = Chat::new(&bytes, chat.id, None, account, again.clone(), files, &chat.core);
        back.persistence = Some(store);
        frames_until(&again, &mut back, |c| c.is_ready());
        rest(&again, &mut back);
        assert!((top_of(&back) - left_at).abs() < 1.0, "{} vs {left_at}", top_of(&back));
    }

    /// Where a finger does the pointing, a tap on a message shows its
    /// actions and a tap elsewhere puts them away.
    #[test]
    fn a_tap_shows_a_messages_actions() {
        let ctx = context();
        ctx.set_os(egui::os::OperatingSystem::IOS);
        let (mut chat, last) = long_chat(&ctx);
        let reply = ctx
            .read_response(egui::Id::new(("chat_text", last)))
            .unwrap()
            .rect;
        assert_eq!(chat.tapped, None);
        click(&ctx, &mut chat, reply.center());
        assert_eq!(chat.tapped, Some(last));
        click(&ctx, &mut chat, pos2(reply.center().x, reply.top() - 60.0));
        assert_ne!(chat.tapped, Some(last));
    }

    /// Jump to latest pressed while the wheel still coasts: the view goes to
    /// the newest line and stays there, and once the wheel has rested it
    /// scrolls again.
    #[test]
    fn jump_to_latest_outruns_scroll_momentum() {
        let (lb, id) = account_with_chat();
        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let me = account.username.clone();
        let mut t = Transcript::default();
        for i in 0..40 {
            t.push(Entry::user(&me, format!("question {i}")));
            t.push(Entry::assistant(&me, format!("answer {i}"), "m", Usage::default()));
        }
        let mut chat = Chat::new(&t.serialize(), id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut chat, |c| c.is_ready());
        // Opening heads for the newest line too; let that rest.
        for _ in 0..20 {
            frame(&ctx, &mut chat, vec![]);
        }
        let jump = || {
            ctx.read_response(egui::Id::new(("chat_jump", id)))
                .map(|r| r.rect.center())
        };
        assert!(jump().is_none(), "a chat opens at its newest line");

        let coast = |points: f32| Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, points),
            modifiers: Modifiers::NONE,
        };
        frame(&ctx, &mut chat, vec![Event::PointerMoved(pos2(450.0, 300.0))]);
        for _ in 0..6 {
            frame(&ctx, &mut chat, vec![coast(80.0)]);
        }
        let button = jump().expect("scrolled up, the button shows");

        // The click lands mid-coast, and the coasting outlasts it.
        let press = |pressed| Event::PointerButton {
            pos: button,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        };
        frame(&ctx, &mut chat, vec![Event::PointerMoved(button), coast(40.0)]);
        frame(&ctx, &mut chat, vec![press(true), coast(40.0)]);
        frame(&ctx, &mut chat, vec![press(false), coast(40.0)]);
        for i in 0..30 {
            frame(&ctx, &mut chat, vec![coast(30.0 - i as f32)]);
        }
        for _ in 0..40 {
            frame(&ctx, &mut chat, vec![]);
        }
        assert!(jump().is_none(), "the jump held against the coasting wheel");

        for _ in 0..4 {
            frame(&ctx, &mut chat, vec![coast(80.0)]);
        }
        assert!(jump().is_some(), "a rested wheel scrolls again");
    }

    /// A long chat between one person and their model, opened and at rest.
    /// Returns the tab and the id of its last reply.
    fn long_chat(ctx: &Context) -> (Chat, Uuid) {
        let (lb, id) = account_with_chat();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let me = account.username.clone();
        let mut t = Transcript::default();
        for i in 0..30 {
            t.push(Entry::user(&me, format!("question {i}")));
            t.push(Entry::assistant(&me, format!("alpha beta gamma {i}"), "m", Usage::default()));
        }
        let last = t.entries.last().unwrap().id;
        let mut chat = Chat::new(&t.serialize(), id, None, account, ctx.clone(), files, &lb);
        frames_until(ctx, &mut chat, |c| c.is_ready());
        for _ in 0..20 {
            frame(ctx, &mut chat, vec![]);
        }
        (chat, last)
    }

    fn button(pos: egui::Pos2, pressed: bool) -> Event {
        Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Modifiers::NONE,
        }
    }

    /// Press at `from`, move to `to`, and let go.
    fn drag(ctx: &Context, chat: &mut Chat, from: egui::Pos2, to: egui::Pos2) {
        frame(ctx, chat, vec![Event::PointerMoved(from)]);
        frame(ctx, chat, vec![button(from, true)]);
        for step in 1..=5 {
            let at = from + (to - from) * (step as f32 / 5.0);
            frame(ctx, chat, vec![Event::PointerMoved(at)]);
        }
        frame(ctx, chat, vec![button(to, false)]);
        frame(ctx, chat, vec![]);
    }

    /// A reply's text selects with the mouse and copies. The next keystroke
    /// lands in the composer, and the selection goes with the keyboard.
    #[test]
    fn a_reply_selects_copies_and_hands_the_keyboard_back() {
        let ctx = context();
        let (mut chat, last) = long_chat(&ctx);
        let text = egui::Id::new(("chat_text", last));
        let rect = ctx.read_response(text).expect("the last reply").rect;

        drag(&ctx, &mut chat, rect.left_center(), rect.right_center());
        assert!(ctx.memory(|m| m.has_focus(text)), "the reply holds the keyboard");
        let selection = |chat: &Chat| chat.readers[&last].renderer.buffer.current.selection;
        assert_ne!(selection(&chat).0, selection(&chat).1, "and a selection");

        let out = ctx.run(raw_input(vec![Event::Copy]), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                chat.show(ui);
            });
        });
        let copied: Vec<_> = out
            .platform_output
            .commands
            .iter()
            .filter_map(|cmd| match cmd {
                egui::OutputCommand::CopyText(text) => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(copied, ["alpha beta gamma 29"]);

        frame(&ctx, &mut chat, vec![Event::Text("x".into())]);
        for _ in 0..3 {
            frame(&ctx, &mut chat, vec![]);
        }
        assert_eq!(chat.composer.renderer.buffer.current.text, "x", "typing lands");
        assert!(!ctx.memory(|m| m.has_focus(text)));
        assert_eq!(selection(&chat).0, selection(&chat).1, "one selection at a time");
    }

    /// A mouse drag over the transcript is not a scroll: the page stays put
    /// under it, where a wheel moves it.
    #[test]
    fn a_mouse_drag_does_not_scroll() {
        let ctx = context();
        let (mut chat, last) = long_chat(&ctx);
        let top = |ctx: &Context| {
            let text = egui::Id::new(("chat_text", last));
            ctx.read_response(text).expect("the last reply").rect.top()
        };
        let before = top(&ctx);

        // The margin beside the column, where no text takes the drag.
        drag(&ctx, &mut chat, pos2(20.0, 200.0), pos2(20.0, 400.0));
        assert_eq!(top(&ctx), before, "a drag left the page where it was");

        let wheel = Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 80.0),
            modifiers: Modifiers::NONE,
        };
        frame(&ctx, &mut chat, vec![Event::PointerMoved(pos2(450.0, 300.0)), wheel]);
        for _ in 0..20 {
            frame(&ctx, &mut chat, vec![]);
        }
        assert!(top(&ctx) > before, "the wheel scrolls");
    }

    /// The transcript fades out over its last stretch above the composer.
    /// At the end of a chat that stretch is blank: the newest row, its
    /// action strip included, ends where the fade begins, which is where
    /// the jump button rests.
    #[test]
    fn the_end_of_a_chat_sits_clear_of_the_fade() {
        let ctx = context();
        let (mut chat, last) = long_chat(&ctx);
        let newest = chat
            .spans
            .iter()
            .find(|(id, _, _)| *id == last)
            .map(|(_, _, bottom)| *bottom)
            .expect("the last reply");

        let wheel = Event::MouseWheel {
            unit: egui::MouseWheelUnit::Point,
            delta: egui::vec2(0.0, 80.0),
            modifiers: Modifiers::NONE,
        };
        frame(&ctx, &mut chat, vec![Event::PointerMoved(pos2(450.0, 300.0)), wheel]);
        for _ in 0..20 {
            frame(&ctx, &mut chat, vec![]);
        }
        let jump = ctx
            .read_response(egui::Id::new(("chat_jump", chat.id)))
            .expect("scrolled up, the button shows");
        let fade_top = jump.rect.bottom();
        assert!(
            (newest - fade_top).abs() < 1.0,
            "newest line ends at {newest}, fade from {fade_top}"
        );
    }

    /// An effort, like a model, is remembered in the chat and becomes what
    /// the next new chat starts with. A new model starts back at its own.
    #[test]
    fn an_effort_sticks_until_the_model_changes() {
        let (lb, id) = account_with_chat();
        let ctx = context();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let mut chat = Chat::new(b"", id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut chat, |c| c.is_ready());
        let default = || -> serde_json::Value {
            let file = lb.get_by_path("/.agent/default.json").unwrap();
            serde_json::from_slice(&lb.read_document(file.id, false).unwrap()).unwrap()
        };

        chat.set_effort(Some("high".into()));
        assert_eq!(chat.settings().effort.as_deref(), Some("high"));
        assert_eq!(
            (default()["provider"].clone(), default()["effort"].clone()),
            ("mock".into(), "high".into())
        );

        chat.select("mock/another".into());
        assert_eq!(chat.settings().effort, None);
        assert_eq!(default()["model"], "another");
        assert_eq!(default()["effort"], serde_json::Value::Null);
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

    /// The composer offers a call only where the chat's model speaks.
    #[test]
    fn a_call_is_offered_only_to_a_model_that_speaks() {
        crate::voice::offer();
        let ctx = context();
        let (lb, id) = account_with_chat();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let empty = Transcript::default().serialize();
        let mut chat = Chat::new(&empty, id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut chat, |c| c.is_ready());
        frame(&ctx, &mut chat, vec![]);
        let call = egui::Id::new(("chat_call", false));
        assert!(ctx.read_response(call).is_none(), "no call for a model that cannot speak");

        chat.select("mock/gpt-realtime".into());
        frames_until(
            &ctx,
            &mut chat,
            |c| matches!(&c.provider, Some(Ok(p)) if p.model == "gpt-realtime"),
        );
        frame(&ctx, &mut chat, vec![]);
        assert!(ctx.read_response(call).is_some(), "a call for a model that speaks");
    }

    /// Putting the keyboard away takes the composer's menu with it.
    #[test]
    fn the_menu_goes_with_the_keyboard() {
        let ctx = context();
        let (lb, id) = account_with_chat();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let empty = Transcript::default().serialize();
        let mut chat = Chat::new(&empty, id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut chat, |c| c.is_ready());
        frame(&ctx, &mut chat, vec![]);
        chat.set_keyboard_shown(true);
        let chip = egui::Id::new(("chat_chip", "model"));
        let at = ctx
            .read_response(chip)
            .expect("the model chip")
            .rect
            .center();
        click(&ctx, &mut chat, at);
        assert!(crate::style::context_menu::is_open_id(&ctx, chip));

        chat.set_keyboard_shown(false);
        frame(&ctx, &mut chat, vec![]);
        assert!(!crate::style::context_menu::is_open_id(&ctx, chip));
    }

    /// A finger on the model button opens the sheet itself, with no menu
    /// between; the sheet leads with the choice and the favorites.
    #[test]
    fn a_finger_on_the_model_button_opens_the_sheet() {
        let ctx = context();
        ctx.set_os(egui::os::OperatingSystem::IOS);
        let (lb, id) = account_with_chat();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let empty = Transcript::default().serialize();
        let mut chat = Chat::new(&empty, id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut chat, |c| c.is_ready());
        frame(&ctx, &mut chat, vec![]);
        // A favorite whose provider is not offered here is left out.
        chat.favorites = vec!["mock/m".into(), "elsewhere/x".into(), "mock/other".into()];
        let chip = egui::Id::new(("chat_chip", "model"));
        let at = ctx
            .read_response(chip)
            .expect("the model chip")
            .rect
            .center();
        click(&ctx, &mut chat, at);
        assert!(chat.models_open, "the sheet is up");
        assert!(!crate::style::context_menu::is_open_id(&ctx, chip), "and no menu");

        let outline: Vec<String> = chat
            .rows()
            .iter()
            .map(|item| match item {
                Item::Current { selection, .. } => format!("current {selection}"),
                Item::Favorites { open } => format!("favorites {open}"),
                Item::Favorite { selection, .. } => format!("  {selection}"),
                Item::Provider { name, .. } => format!("provider {name}"),
                Item::AddProvider => "add a provider".into(),
                other => format!("{other:?}"),
            })
            .collect();
        let expected = [
            "current mock/m",
            "favorites true",
            "  mock/m",
            "  mock/other",
            "add a provider",
            "provider mock",
        ];
        assert_eq!(&outline[..6], &expected, "no thinking row for a model that has no levels");
        let listed = ["m", "other"].map(|id| lb_chat::ModelInfo {
            id: id.into(),
            display_name: None,
            window: None,
        });
        chat.listings
            .insert("mock".into(), ListingState::Ready(listed.to_vec()));
        chat.model_filter = "oth".into();
        assert!(matches!(chat.rows()[0], Item::Provider { .. }), "a filter leaves the tree alone");
    }

    /// On a phone the text sits beside the buttons while it fits there on
    /// one line, and takes the full width once it does not.
    #[test]
    fn a_long_draft_takes_the_full_width_on_a_phone() {
        let ctx = context();
        let (lb, id) = account_with_chat();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let mut chat = Chat::new(b"", id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut chat, |c| c.is_ready());
        let text_rect = |chat: &mut Chat| {
            let phone = RawInput {
                screen_rect: Some(Rect::from_min_max(pos2(0.0, 0.0), pos2(390.0, 700.0))),
                ..Default::default()
            };
            let mut out = Rect::NOTHING;
            for _ in 0..2 {
                let _ = ctx.run(phone.clone(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        out = chat.show(ui).0;
                    });
                });
            }
            out
        };
        chat.composer.set_text("Short");
        let short = text_rect(&mut chat);
        chat.composer
            .set_text(&"a long draft that wraps ".repeat(4));
        let long = text_rect(&mut chat);
        assert!(long.width() > short.width() + 60.0, "{short:?} {long:?}");
    }

    /// A new chat reads the folder the most recently changed chat beside it
    /// reads.
    #[test]
    fn a_new_chat_reads_what_the_latest_beside_it_does() {
        let ctx = context();
        let (lb, _) = account_with_chat();
        let me = lb.get_account().unwrap().username.clone();
        let mut earlier = Transcript::default();
        let include = vec!["/notes/".to_string()];
        let settings =
            lb_rs::model::chat::Settings { include: include.clone(), ..Default::default() };
        earlier.set_settings(&me, settings);
        let bytes = earlier.serialize();
        write(&lb, "/convos/earlier.chat", std::str::from_utf8(&bytes).unwrap());
        let new = lb.create_at_path("/convos/new.chat").unwrap();
        let files = Arc::new(RwLock::new(FileCache::new(&lb).unwrap()));
        let account = lb.get_account().unwrap().clone();
        let mut chat = Chat::new(b"", new.id, None, account, ctx.clone(), files, &lb);
        frames_until(&ctx, &mut chat, |c| c.settings().include == include);
    }
}
