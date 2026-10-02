//! The librarian against a real vault. Needs a local server
//! (`lbdev ci start-server`), like the lb-rs integration tests.

use lb_chat::{Call, Provider, ToolOutcome, Tools, VaultTools};
use lb_rs::blocking::Lb;
use lb_rs::model::chat::{Chat, Entry, Settings};
use serde_json::{Value, json};
use test_utils::{random_name, test_config, url};

fn account() -> Lb {
    let lb = Lb::init(test_config()).unwrap();
    lb.create_account(&random_name(), &url(), false).unwrap();
    lb
}

fn write(lb: &Lb, path: &str, text: &str) {
    let file = lb.create_at_path(path).unwrap();
    lb.write_document(file.id, text.as_bytes()).unwrap();
}

fn call(tools: &mut VaultTools, name: &str, args: Value) -> ToolOutcome {
    tools.call(&Call { id: "c".into(), name: name.into(), args, echo: None })
}

fn done(outcome: ToolOutcome) -> (String, bool) {
    match outcome {
        ToolOutcome::Done { text, ok } => (text, ok),
        ToolOutcome::Abort { text } => panic!("unexpected abort: {text}"),
    }
}

#[test]
fn librarian_over_a_small_vault() {
    let lb = account();
    write(&lb, "/home/todo.md", "# Todo\n\n- milk\n");
    write(&lb, "/home/notes/plan.md", "# Plan\n\nship chat\n\n## Later\n\nvoice\n");
    write(&lb, "/team/public.md", "hello team");
    write(&lb, "/team/secret/keys.md", "k");
    write(&lb, "/team/.agentignore", "secret/\n");
    write(&lb, "/elsewhere/x.md", "x marks");

    let mut chat = Chat::default();
    chat.set_settings("u", Settings { include: vec!["/team/".into()], ..Default::default() });
    let own = lb.create_at_path("/home/talk.chat").unwrap();
    let mut tools = VaultTools::new(lb.clone(), own.id);
    tools.prepare(&chat, "u", "/home/");

    assert_eq!(done(call(&mut tools, "list", json!({}))).0, "notes/\ntodo.md");
    assert_eq!(done(call(&mut tools, "list", json!({"path": "/team/"}))).0, "public.md");

    let (section, ok) =
        done(call(&mut tools, "read", json!({"path": "/home/notes/plan.md", "section": "later"})));
    assert!(ok && section == "## Later\n\nvoice", "{section}");

    let (hits, _) = done(call(&mut tools, "search", json!({"query": "ship chat"})));
    assert!(hits.contains("/home/notes/plan.md") && hits.contains("ship chat"), "{hits}");
    let (hits, _) = done(call(&mut tools, "search", json!({"query": "k", "folder": "/team"})));
    assert!(!hits.contains("secret"), "{hits}");

    let (text, ok) = done(call(&mut tools, "read", json!({"path": "/team/secret/keys.md"})));
    assert!(!ok && text.contains("does not exist"), "{text}");
    let (text, ok) = done(call(&mut tools, "read", json!({"path": "/elsewhere/x.md"})));
    assert!(!ok && text.contains("outside"), "{text}");

    assert!(matches!(
        call(&mut tools, "create", json!({"path": "/team/secret/new.md"})),
        ToolOutcome::Abort { .. }
    ));
    assert!(matches!(
        call(&mut tools, "move", json!({"path": "/home/todo.md", "to": "/team/secret/todo.md"})),
        ToolOutcome::Abort { .. }
    ));

    let (text, ok) = done(call(
        &mut tools,
        "edit",
        json!({"path": "/home/todo.md", "old": "- milk", "new": "- milk\n- eggs"}),
    ));
    assert!(ok, "{text}");
    let (text, _) = done(call(&mut tools, "read", json!({"path": "/home/todo.md"})));
    assert!(text.contains("- eggs"));
    let (text, ok) =
        done(call(&mut tools, "edit", json!({"path": "/home/todo.md", "old": "nope", "new": ""})));
    assert!(!ok && text.contains("not found"), "{text}");

    let (_, ok) =
        done(call(&mut tools, "create", json!({"path": "/home/new.md", "text": "fresh"})));
    assert!(ok);
    let (_, ok) = done(call(
        &mut tools,
        "move",
        json!({"path": "/home/new.md", "to": "/home/notes/renamed.md"}),
    ));
    assert!(ok);
    assert_eq!(
        done(call(&mut tools, "list", json!({"path": "/home/notes"}))).0,
        "plan.md\nrenamed.md"
    );

    // A delete goes through at once: the folder is the only limit.
    let (text, ok) = done(call(&mut tools, "delete", json!({"path": "/home/notes/renamed.md"})));
    assert!(ok, "{text}");
    assert_eq!(done(call(&mut tools, "list", json!({"path": "/home/notes"}))).0, "plan.md");

    // Out of reach stays out of reach: no tool widens it.
    let ask = json!({"path": "/elsewhere/", "reason": "x"});
    let (text, ok) = done(call(&mut tools, "request_access", ask));
    assert!(!ok && text.contains("no tool named"), "{text}");
    let (text, ok) = done(call(&mut tools, "read", json!({"path": "/elsewhere/x.md"})));
    assert!(!ok && text.contains("outside") && !text.contains("request_access"), "{text}");
}

#[test]
fn a_chat_does_not_exist_to_its_own_tools_and_other_chats_do() {
    let lb = account();
    let mut said = Chat::default();
    said.push(Entry::user("u", "zebra crossing"));
    let transcript = String::from_utf8(said.serialize()).unwrap();
    write(&lb, "/home/talk.chat", &transcript);
    write(&lb, "/home/other.chat", &transcript);
    write(&lb, "/home/todo.md", "milk");
    let own = lb.get_by_path("/home/talk.chat").unwrap();
    let mut tools = VaultTools::new(lb.clone(), own.id);
    tools.prepare(&Chat::default(), "u", "/home/");

    assert_eq!(done(call(&mut tools, "list", json!({}))).0, "other.chat\ntodo.md");
    let (hits, _) = done(call(&mut tools, "search", json!({"query": "zebra"})));
    assert!(hits.contains("/home/other.chat") && !hits.contains("talk"), "{hits}");
    assert_eq!(done(call(&mut tools, "search", json!({"query": "talk"}))).0, "no matches");
    let (text, ok) = done(call(&mut tools, "read", json!({"path": "/home/other.chat"})));
    assert!(ok && text.contains("zebra crossing"), "{text}");

    for (tool, args) in [
        ("read", json!({"path": "/home/talk.chat"})),
        ("list", json!({"path": "/home/talk.chat"})),
        ("edit", json!({"path": "/home/talk.chat", "old": "zebra", "new": "horse"})),
        ("move", json!({"path": "/home/talk.chat", "to": "/home/moved.chat"})),
        ("delete", json!({"path": "/home/talk.chat"})),
    ] {
        let (text, ok) = done(call(&mut tools, tool, args));
        assert!(!ok && text.ends_with("does not exist"), "{tool}: {text}");
    }

    // Its path is refused without ending the run, and it is left as it was.
    let (text, ok) = done(call(&mut tools, "create", json!({"path": "/home/talk.chat"})));
    assert!(!ok && text.contains("is taken"), "{text}");
    let onto = json!({"path": "/home/todo.md", "to": "/home/talk.chat"});
    let (text, ok) = done(call(&mut tools, "move", onto));
    assert!(!ok && text.contains("is taken"), "{text}");
    assert_eq!(lb.get_path_by_id(own.id).unwrap(), "/home/talk.chat");
    assert_eq!(lb.read_document(own.id, false).unwrap(), transcript.as_bytes());

    // It stays hidden where the user moves it.
    lb.rename_file(&own.id, "renamed.chat").unwrap();
    tools.prepare(&Chat::default(), "u", "/home/");
    assert_eq!(done(call(&mut tools, "list", json!({}))).0, "other.chat\ntodo.md");
    let (text, ok) = done(call(&mut tools, "read", json!({"path": "/home/renamed.chat"})));
    assert!(!ok && text.ends_with("does not exist"), "{text}");
}

/// A chat's effort is its own choice, else the default's while it follows
/// the default's model, and never a value its model has not been shown to
/// take.
#[test]
fn a_chat_resolves_its_effort() {
    let lb = account();
    let grok = r#"{"base_url": "https://api.x.ai/v1", "model": "grok-4.7", "api_key": "k"}"#;
    write(&lb, "/.agent/providers/xai.json", grok);
    let default = r#"{"provider": "xai", "model": "grok-4.7", "effort": "high"}"#;
    write(&lb, "/.agent/default.json", default);
    let effort = |settings: Settings| Provider::resolve(&lb, &settings).unwrap().effort;
    let chose = |effort: &str| Settings { effort: Some(effort.into()), ..Default::default() };

    assert_eq!(effort(Settings::default()).as_deref(), Some("high"));
    assert_eq!(effort(chose("low")).as_deref(), Some("low"));
    assert_eq!(effort(chose("enormous")), None);
    // Its own model, so not the default's effort; and one nothing is known of.
    let own = |model: &str| Settings { model: Some(model.into()), ..Default::default() };
    assert_eq!(effort(own("xai/grok-4.7")), None);
    assert_eq!(effort(Settings { model: Some("xai/grok-9".into()), ..chose("low") }), None);
}

/// A folder's standing instructions are its `AGENTS.md`, gathered from the
/// root down to the chat's folder, whoever's reach they are in or out of.
#[test]
fn instructions_come_from_the_root_down_to_the_chats_folder() {
    let lb = account();
    write(&lb, "/AGENTS.md", "be brief");
    write(&lb, "/home/AGENTS.md", "  ");
    write(&lb, "/home/trips/AGENTS.md", "dates are ISO");
    write(&lb, "/home/trips/deeper/AGENTS.md", "not for this chat");
    write(&lb, "/elsewhere/AGENTS.md", "nor this");
    let own = lb.create_at_path("/home/trips/talk.chat").unwrap();
    let mut tools = VaultTools::new(lb.clone(), own.id);
    let found = tools.instructions("/home/trips/");
    let expected = [("/AGENTS.md", "be brief"), ("/home/trips/AGENTS.md", "dates are ISO")];
    let found: Vec<(&str, &str)> = found
        .iter()
        .map(|(p, t)| (p.as_str(), t.as_str()))
        .collect();
    assert_eq!(found, expected);
}

/// A long note with no headings cannot be read by section, so it is read
/// from its start, and asking for a section says why there is none.
#[test]
fn a_long_note_without_headings_reads_from_its_start() {
    let lb = account();
    write(&lb, "/home/log.md", &"a line of the log\n".repeat(3000));
    let own = lb.create_at_path("/home/talk.chat").unwrap();
    let mut tools = VaultTools::new(lb.clone(), own.id);
    tools.prepare(&Chat::default(), "u", "/home/");
    let (text, ok) = done(call(&mut tools, "read", json!({"path": "/home/log.md"})));
    assert!(ok && text.starts_with("a line of the log") && text.ends_with("more bytes not shown)"));
    let args = json!({"path": "/home/log.md", "section": "Monday"});
    let (text, ok) = done(call(&mut tools, "read", args));
    assert!(!ok && text.contains("has no headings"), "{text}");
}

/// A search is offered once an engine is set up, and a page is fetched only
/// from an address the chat was given, not one the model made up.
#[test]
fn the_web_is_reached_by_what_the_user_set_up_and_gave() {
    let lb = account();
    let own = lb.create_at_path("/home/talk.chat").unwrap();
    let mut tools = VaultTools::new(lb.clone(), own.id);
    let names = |tools: &VaultTools| -> Vec<String> {
        tools.schemas().into_iter().map(|s| s.name).collect()
    };
    let mut chat = Chat::default();
    tools.prepare(&chat, "u", "/home/");
    assert!(names(&tools).contains(&"fetch".to_string()));
    assert!(!names(&tools).contains(&"web_search".to_string()));

    write(&lb, "/.agent/search/brave.json", r#"{"kind":"brave","api_key":"k"}"#);
    chat.push(Entry::user("u", "see http://127.0.0.1:9/page for it"));
    tools.prepare(&chat, "u", "/home/");
    assert!(names(&tools).contains(&"web_search".to_string()));

    let made_up = json!({"url": "http://127.0.0.1:9/page?leak=secret"});
    let (text, ok) = done(call(&mut tools, "fetch", made_up));
    assert!(!ok && text.contains("is not an address the user gave"), "{text}");
    let given = json!({"url": "http://127.0.0.1:9/page"});
    let (text, ok) = done(call(&mut tools, "fetch", given));
    assert!(!ok && text.contains("can't reach"), "{text}");
}

/// A picture is read as its size, and shown at about a megapixel; a drawing
/// is drawn first. What is no picture, or out of reach, is not shown.
#[test]
fn a_picture_is_read_as_its_size_and_shown_small() {
    let lb = account();
    let own = lb.create_at_path("/home/talk.chat").unwrap();
    let put = |path: &str, bytes: &[u8]| {
        let file = lb.create_at_path(path).unwrap();
        lb.write_document(file.id, bytes).unwrap();
    };
    let mut photo = std::io::Cursor::new(Vec::new());
    image::DynamicImage::new_rgb8(2000, 1500)
        .write_to(&mut photo, image::ImageFormat::Png)
        .unwrap();
    put("/home/photo.png", &photo.into_inner());
    put("/home/fake.png", b"not a picture");
    put("/away/photo.png", b"");
    let drawing = r#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="30"><rect width="10" height="10"/></svg>"#;
    put("/home/drawing.svg", drawing.as_bytes());
    let mut tools = VaultTools::new(lb.clone(), own.id);
    tools.prepare(&Chat::default(), "u", "/home/");

    let (text, ok) = done(call(&mut tools, "read", json!({"path": "/home/photo.png"})));
    assert!(ok && text.contains("is a picture, 2000 by 1500"), "{text}");
    let shown = tools.media("/home/photo.png").unwrap();
    let bytes = base64::decode(&shown.data).unwrap();
    let small = image::load_from_memory(&bytes).unwrap();
    assert_eq!(shown.mime, "image/jpeg");
    assert!(small.width() * small.height() <= 1024 * 1024 && small.width() > 1000);

    assert!(tools.media("/home/drawing.svg").is_some());
    let (text, ok) = done(call(&mut tools, "read", json!({"path": "/home/fake.png"})));
    assert!(!ok && text.contains("could not be read as a picture"), "{text}");
    assert!(tools.media("/away/photo.png").is_none());

    // A PDF is read as its size and sent whole.
    put("/home/paper.pdf", b"%PDF-1.4 and so on");
    let (text, ok) = done(call(&mut tools, "read", json!({"path": "/home/paper.pdf"})));
    assert!(ok && text.contains("is a PDF"), "{text}");
    let sent = tools.media("/home/paper.pdf").unwrap();
    assert_eq!(sent.mime, "application/pdf");
    assert_eq!(base64::decode(&sent.data).unwrap(), b"%PDF-1.4 and so on");
}

/// What the model made is kept in the chat's imports folder, and a name
/// already taken there gets a number.
#[test]
fn what_was_made_is_kept_beside_the_chat() {
    let lb = account();
    let own = lb.create_at_path("/home/talk.chat").unwrap();
    let mut tools = VaultTools::new(lb.clone(), own.id);
    tools.prepare(&Chat::default(), "u", "/home/");
    assert_eq!(tools.keep("a.png", b"one").unwrap(), "/home/imports/a.png");
    assert_eq!(tools.keep("a.png", b"two").unwrap(), "/home/imports/a-2.png");
    let second = lb.get_by_path("/home/imports/a-2.png").unwrap();
    assert_eq!(lb.read_document(second.id, false).unwrap(), b"two");
}

/// A recording is read as the transcript note beside it; with none there
/// and nobody set up to write one, the read says so.
#[test]
fn a_recording_is_read_as_its_transcript() {
    let lb = account();
    let own = lb.create_at_path("/home/talk.chat").unwrap();
    let memo = lb.create_at_path("/home/memo.m4a").unwrap();
    lb.write_document(memo.id, b"sound").unwrap();
    let mut tools = VaultTools::new(lb.clone(), own.id);
    tools.prepare(&Chat::default(), "u", "/home/");

    let (text, ok) = done(call(&mut tools, "read", json!({"path": "/home/memo.m4a"})));
    assert!(!ok && text.contains("no provider that transcribes"), "{text}");
    write(&lb, "/home/memo.m4a.transcript.md", "buy milk");
    let (text, ok) = done(call(&mut tools, "read", json!({"path": "/home/memo.m4a"})));
    assert_eq!((text.as_str(), ok), ("buy milk", true));
}
