//! The librarian against a real vault. Needs a local server
//! (`lbdev ci start-server`), like the lb-rs integration tests.

use lb_chat::{Call, Provider, ToolOutcome, Tools, VaultTools};
use lb_rs::blocking::Lb;
use lb_rs::model::chat::{Chat, Settings};
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
    let mut tools = VaultTools::new(lb.clone());
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
