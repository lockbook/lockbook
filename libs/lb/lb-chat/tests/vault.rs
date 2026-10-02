//! The librarian against a real vault. Needs a local server
//! (`lbdev ci start-server`), like the lb-rs integration tests.

use lb_chat::{Call, ToolOutcome, Tools, VaultTools};
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

fn call(tools: &mut VaultTools, name: &str, args: Value, approved: bool) -> ToolOutcome {
    tools.call(&Call { id: "c".into(), name: name.into(), args }, approved)
}

fn done(outcome: ToolOutcome) -> (String, bool) {
    match outcome {
        ToolOutcome::Done { text, ok } => (text, ok),
        ToolOutcome::Ask { prompt } => panic!("unexpected ask: {prompt}"),
        ToolOutcome::Grant { text, .. } => panic!("unexpected grant: {text}"),
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
    chat.push(Entry::settings(
        "u",
        Settings { include: vec!["/team/".into()], ..Default::default() },
    ));
    let mut tools = VaultTools::new(lb.clone());
    tools.prepare(&chat, "u", "/home/");

    assert_eq!(done(call(&mut tools, "list", json!({}), false)).0, "notes/\ntodo.md");
    assert_eq!(done(call(&mut tools, "list", json!({"path": "/team/"}), false)).0, "public.md");

    let (section, ok) = done(call(
        &mut tools,
        "read",
        json!({"path": "/home/notes/plan.md", "section": "later"}),
        false,
    ));
    assert!(ok && section == "## Later\n\nvoice", "{section}");

    let (hits, _) = done(call(&mut tools, "search", json!({"query": "ship chat"}), false));
    assert!(hits.contains("/home/notes/plan.md") && hits.contains("ship chat"), "{hits}");
    let (hits, _) =
        done(call(&mut tools, "search", json!({"query": "k", "folder": "/team"}), false));
    assert!(!hits.contains("secret"), "{hits}");

    let (text, ok) = done(call(&mut tools, "read", json!({"path": "/team/secret/keys.md"}), false));
    assert!(!ok && text.contains("does not exist"), "{text}");
    let (text, ok) = done(call(&mut tools, "read", json!({"path": "/elsewhere/x.md"}), false));
    assert!(!ok && text.contains("outside"), "{text}");

    assert!(matches!(
        call(&mut tools, "create", json!({"path": "/team/secret/new.md"}), false),
        ToolOutcome::Abort { .. }
    ));
    assert!(matches!(
        call(
            &mut tools,
            "move",
            json!({"path": "/home/todo.md", "to": "/team/secret/todo.md"}),
            false
        ),
        ToolOutcome::Abort { .. }
    ));

    let (text, ok) = done(call(
        &mut tools,
        "edit",
        json!({"path": "/home/todo.md", "old": "- milk", "new": "- milk\n- eggs"}),
        false,
    ));
    assert!(ok, "{text}");
    let (text, _) = done(call(&mut tools, "read", json!({"path": "/home/todo.md"}), false));
    assert!(text.contains("- eggs"));
    let (text, ok) = done(call(
        &mut tools,
        "edit",
        json!({"path": "/home/todo.md", "old": "nope", "new": ""}),
        false,
    ));
    assert!(!ok && text.contains("not found"), "{text}");

    let (_, ok) =
        done(call(&mut tools, "create", json!({"path": "/home/new.md", "text": "fresh"}), false));
    assert!(ok);
    let (_, ok) = done(call(
        &mut tools,
        "move",
        json!({"path": "/home/new.md", "to": "/home/notes/renamed.md"}),
        false,
    ));
    assert!(ok);
    assert_eq!(
        done(call(&mut tools, "list", json!({"path": "/home/notes"}), false)).0,
        "plan.md\nrenamed.md"
    );

    assert!(matches!(
        call(&mut tools, "delete", json!({"path": "/home/notes/renamed.md"}), false),
        ToolOutcome::Ask { .. }
    ));
    let (_, ok) = done(call(&mut tools, "delete", json!({"path": "/home/notes/renamed.md"}), true));
    assert!(ok);
    assert_eq!(done(call(&mut tools, "list", json!({"path": "/home/notes"}), false)).0, "plan.md");

    assert!(matches!(
        call(&mut tools, "request_access", json!({"path": "/elsewhere/", "reason": "x"}), false),
        ToolOutcome::Ask { .. }
    ));
    assert!(matches!(
        call(&mut tools, "request_access", json!({"path": "/elsewhere/", "reason": "x"}), true),
        ToolOutcome::Grant { .. }
    ));
    let (text, ok) = done(call(&mut tools, "read", json!({"path": "/elsewhere/x.md"}), false));
    assert!(ok && text == "x marks");

    let (text, ok) = done(call(
        &mut tools,
        "request_access",
        json!({"path": "/team/secret/", "reason": "x"}),
        false,
    ));
    assert!(!ok && text.contains("does not exist"), "{text}");
}
