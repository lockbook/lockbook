//! How a tool call reads in the transcript: an icon, a statement whose
//! variables stand out, and what its card holds when opened.

use lb_rs::Uuid;
use serde_json::Value;

use super::diff::{self, Change};
use crate::style::phosphor;

pub struct Row {
    pub icon: &'static str,
    /// The statement in order. A variable (a path, a query) is marked and
    /// drawn bold.
    pub words: Vec<(String, bool)>,
}

/// One piece of an opened card, top to bottom.
#[derive(Clone, Debug, PartialEq)]
pub enum Part {
    /// Files a search or a listing found; a click opens one.
    Files(Vec<FileRef>),
    /// A note's text, drawn as the note would be.
    Note(String),
    /// Text that is not a note.
    Text(String),
    /// What an edit changed, word by word, with a little context.
    Diff(Vec<Change>),
    /// One quiet line: a confirmation, a count, a failure's reason.
    Line(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileRef {
    /// Absolute; a folder's ends in a slash.
    pub path: String,
    /// The text around a search match, when there is one.
    pub snippet: String,
    /// The note a click opens; the tab finds it when the card opens.
    pub target: Option<Uuid>,
}

/// Files shown before the rest become a count.
const FILES_SHOWN: usize = 20;
/// Lines of a note or of other text shown before the rest become a count:
/// a card previews, and the note's own tab is the full view.
const TEXT_LINES: usize = 24;
/// Words of unchanged text kept on each side of a change.
const DIFF_CONTEXT: usize = 12;

fn arg<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
}

pub fn row(name: &str, args: &Value) -> Row {
    let path = arg(args, "path");
    let folder = path.is_some_and(|p| p.ends_with('/'));
    let (icon, verb) = match name {
        "search" => (phosphor::SEARCH, "search"),
        "read" => (phosphor::FILE_TEXT, "read"),
        "list" => (phosphor::LIST_BULLETS, "list"),
        "edit" => (phosphor::PENCIL, "edit"),
        "create" if folder => (phosphor::FOLDER_PLUS, "create"),
        "create" => (phosphor::FILE_PLUS, "create"),
        "move" => (phosphor::FOLDER, "move"),
        "delete" => (phosphor::TRASH, "delete"),
        // What a provider ran itself.
        "web_search" => (phosphor::GLOBE, "search the web"),
        "x_search" => (phosphor::GLOBE, "search X"),
        "code" => (phosphor::CODE, "run code"),
        "fetch" => (phosphor::GLOBE, "fetch"),
        // Not calls: what a reply showed of its thinking, live and settled.
        "thinking" | "thought" => (phosphor::LIGHTBULB, name),
        other => (phosphor::GEAR, other),
    };
    let mut words = vec![(verb.to_string(), false)];
    let mut variable = |text: String| words.push((text, true));
    match name {
        "search" | "web_search" | "x_search" => arg(args, "query")
            .map(str::to_string)
            .into_iter()
            .for_each(&mut variable),
        "fetch" => arg(args, "url")
            .map(str::to_string)
            .into_iter()
            .for_each(&mut variable),
        // A section reads as a link's fragment does.
        "read" => match (path, arg(args, "section")) {
            (Some(path), Some(section)) => variable(format!("{path}#{section}")),
            (Some(path), None) => variable(path.to_string()),
            _ => {}
        },
        _ => path.map(str::to_string).into_iter().for_each(&mut variable),
    }
    if name == "move" {
        if let Some(to) = arg(args, "to") {
            words.push(("to".to_string(), false));
            words.push((to.to_string(), true));
        }
    }
    Row { icon, words }
}

/// Shortens the outermost folder of `path` still spelled out to its first
/// letter: `/projects/work/note.md` is `/p/work/note.md`, then
/// `/p/w/note.md`. The last name is never shortened, nor is anything after
/// a `#`. Nothing when no folder is left to shorten.
pub fn abbreviate(path: &str) -> Option<String> {
    let (path, fragment) = match path.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (path, None),
    };
    let mut parts: Vec<String> = path.split('/').map(str::to_string).collect();
    let leaf = parts.iter().rposition(|p| !p.is_empty())?;
    let folder = parts[..leaf]
        .iter()
        .position(|p| p.chars().count() > 1 && p != "..")?;
    parts[folder] = parts[folder].chars().take(1).collect();
    let path = parts.join("/");
    Some(match fragment {
        Some(fragment) => format!("{path}#{fragment}"),
        None => path,
    })
}

/// What an opened card holds. `working_dir` is where a `list` with no path
/// looked. Never empty: every call opens onto something.
pub fn body(name: &str, args: &Value, result: &str, ok: bool, working_dir: &str) -> Vec<Part> {
    let result = result.trim();
    let mut parts = Vec::new();
    // Why it failed comes before what it tried.
    if !ok {
        parts.extend(answer(result));
    }
    if name == "edit" {
        let old = arg(args, "old").unwrap_or_default();
        let new = arg(args, "new").unwrap_or_default();
        if !old.is_empty() || !new.is_empty() {
            parts.push(Part::Diff(diff::in_context(diff::changes(old, new), DIFF_CONTEXT)));
        }
    }
    if !ok {
        return parts;
    }
    let is_note = |path: &str| path.ends_with(".md") || path.ends_with(".chat");
    match name {
        "search" => parts.extend(hits(result)),
        "list" => {
            let folder = arg(args, "path").unwrap_or(working_dir);
            let folder = format!("{}/", folder.trim_end_matches('/'));
            parts.extend(listing(result, &folder));
        }
        "read" if arg(args, "path").is_some_and(is_note) => {
            parts.extend(clipped(result, Part::Note))
        }
        // The diff says it; only a count of more than one adds anything.
        "edit" => parts.extend(
            result
                .split_once('(')
                .map(|(_, count)| count.trim_end_matches(')'))
                .filter(|count| !count.starts_with("1 "))
                .map(|count| Part::Line(count.to_string())),
        ),
        "thinking" | "thought" => parts.push(Part::Note(result.to_string())),
        // Sources, as the list of links they came as.
        "web_search" | "x_search" if !result.is_empty() => {
            parts.push(Part::Note(result.to_string()))
        }
        // Ours returns the page; a provider's own only says it went.
        "fetch" if result != "done" => parts.extend(clipped(result, Part::Text)),
        "code" => {
            let code = arg(args, "code").unwrap_or_default();
            parts.push(Part::Note(format!("```\n{code}\n```")));
            if !result.is_empty() {
                parts.push(Part::Text(result.to_string()));
            }
        }
        "create" => match arg(args, "text") {
            Some(text) if arg(args, "path").is_some_and(is_note) => {
                parts.extend(clipped(text, Part::Note))
            }
            Some(text) => parts.extend(clipped(text, Part::Text)),
            None => parts.extend(answer(result)),
        },
        _ => parts.extend(answer(result)),
    }
    if parts.is_empty() {
        parts.push(Part::Line("done".into()));
    }
    parts
}

/// A tool's own words: one line stays a line, more is text.
fn answer(result: &str) -> Vec<Part> {
    if result.is_empty() {
        vec![Part::Line("done".into())]
    } else if result.contains('\n') {
        clipped(result, Part::Text)
    } else {
        vec![Part::Line(result.to_string())]
    }
}

/// The first lines of `text` as a part, then how many more there are.
fn clipped(text: &str, part: fn(String) -> Part) -> Vec<Part> {
    let lines: Vec<&str> = text.lines().collect();
    let shown = lines.len().min(TEXT_LINES);
    let mut parts = vec![part(lines[..shown].join("\n"))];
    if lines.len() > shown {
        parts.push(Part::Line(format!("{} more lines", lines.len() - shown)));
    }
    parts
}

/// A search's answer: a path per hit with its snippet indented under it,
/// then a line in parentheses when there were more.
fn hits(result: &str) -> Vec<Part> {
    let mut files: Vec<FileRef> = Vec::new();
    let mut lines = Vec::new();
    for line in result.lines() {
        if let Some(snippet) = line.strip_prefix("  ") {
            if let Some(file) = files.last_mut() {
                file.snippet = snippet.trim().to_string();
            }
        } else if line.starts_with('/') {
            files.push(FileRef { path: line.to_string(), snippet: String::new(), target: None });
        } else if !line.trim().is_empty() {
            lines.push(Part::Line(unparenthesized(line)));
        }
    }
    files_then(files, lines)
}

/// A listing's answer: a name per line, folders ending in a slash.
fn listing(result: &str, folder: &str) -> Vec<Part> {
    let mut files = Vec::new();
    let mut lines = Vec::new();
    for line in result.lines().filter(|l| !l.trim().is_empty()) {
        if line.starts_with('(') {
            lines.push(Part::Line(unparenthesized(line)));
        } else {
            let path = format!("{folder}{line}");
            files.push(FileRef { path, snippet: String::new(), target: None });
        }
    }
    files_then(files, lines)
}

/// `text` as spans with every occurrence of a word of `query` marked, as
/// the search page marks its matches. Case is ignored.
pub fn marked(text: &str, query: &str) -> Vec<(String, bool)> {
    let lower = text.to_lowercase();
    let mut marks = vec![false; text.len()];
    // Lowercasing can change a string's length; then nothing is marked.
    if lower.len() == text.len() {
        for word in query.to_lowercase().split_whitespace() {
            for (at, _) in lower.match_indices(word) {
                marks[at..at + word.len()].fill(true);
            }
        }
    }
    let mut spans: Vec<(String, bool)> = Vec::new();
    for (i, ch) in text.char_indices() {
        match spans.last_mut() {
            Some((span, mark)) if *mark == marks[i] => span.push(ch),
            _ => spans.push((ch.to_string(), marks[i])),
        }
    }
    spans
}

fn files_then(mut files: Vec<FileRef>, mut lines: Vec<Part>) -> Vec<Part> {
    let more = files.len().saturating_sub(FILES_SHOWN);
    files.truncate(FILES_SHOWN);
    if more > 0 {
        lines.insert(0, Part::Line(format!("{more} more")));
    }
    let mut parts = Vec::new();
    if !files.is_empty() {
        parts.push(Part::Files(files));
    }
    parts.extend(lines);
    parts
}

fn unparenthesized(line: &str) -> String {
    line.trim()
        .trim_start_matches('(')
        .trim_end_matches(')')
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A row's statement with its variables in brackets.
    fn statement(name: &str, args: Value) -> String {
        let words: Vec<String> = row(name, &args)
            .words
            .into_iter()
            .map(|(text, variable)| if variable { format!("[{text}]") } else { text })
            .collect();
        words.join(" ")
    }

    fn file(path: &str, snippet: &str) -> FileRef {
        FileRef { path: path.into(), snippet: snippet.into(), target: None }
    }

    fn line(text: &str) -> Part {
        Part::Line(text.into())
    }

    #[test]
    fn rows_read_as_a_statement_about_their_variables() {
        assert_eq!(
            statement("read", json!({"path": "/a.md", "section": "Plan"})),
            "read [/a.md#Plan]"
        );
        assert_eq!(statement("read", json!({"path": "/a.md"})), "read [/a.md]");
        assert_eq!(
            statement("move", json!({"path": "/a.md", "to": "/b/a.md"})),
            "move [/a.md] to [/b/a.md]"
        );
        assert_eq!(statement("search", json!({"query": "budget"})), "search [budget]");
        assert_eq!(statement("search", json!({})), "search");
        assert_eq!(statement("frobnicate", json!({"path": "/x"})), "frobnicate [/x]");
        assert_eq!(row("create", &json!({"path": "/x/"})).icon, phosphor::FOLDER_PLUS);
        assert_eq!(row("create", &json!({"path": "/x.md"})).icon, phosphor::FILE_PLUS);
    }

    /// Folders give up all but their first letter from the outside in; the
    /// last name and a fragment stay whole.
    #[test]
    fn a_path_shortens_folder_by_folder() {
        let steps = |path: &str| {
            let mut steps = Vec::new();
            let mut path = path.to_string();
            while let Some(shorter) = abbreviate(&path) {
                steps.push(shorter.clone());
                path = shorter;
            }
            steps
        };
        assert_eq!(
            steps("/projects/work/clients/note.md"),
            ["/p/work/clients/note.md", "/p/w/clients/note.md", "/p/w/c/note.md"]
        );
        assert_eq!(steps("/a/books/drafts/"), ["/a/b/drafts/"]);
        assert_eq!(steps("/books/x.md#Close/doors"), ["/b/x.md#Close/doors"]);
        assert_eq!(steps("/note.md"), Vec::<String>::new());
        assert_eq!(steps("budget"), Vec::<String>::new());
    }

    #[test]
    fn a_search_opens_onto_the_files_it_found() {
        let result = "/home/plan.md\n  we ship chat in october\n/home/todo.md\n(3 more; narrow the query or pass a folder)";
        assert_eq!(
            body("search", &json!({"query": "chat"}), result, true, "/home/"),
            [
                Part::Files(vec![
                    file("/home/plan.md", "we ship chat in october"),
                    file("/home/todo.md", "")
                ]),
                line("3 more; narrow the query or pass a folder"),
            ]
        );
        assert_eq!(body("search", &json!({}), "no matches", true, "/"), [line("no matches")]);
    }

    /// What a reply thought is a row like a call's, and opens onto all of it.
    #[test]
    fn a_thought_opens_onto_all_of_it() {
        assert_eq!(statement("thought", Value::Null), "thought");
        let long = vec!["a line"; TEXT_LINES + 5].join("\n");
        assert_eq!(body("thought", &Value::Null, &long, true, "/"), [Part::Note(long.clone())]);
    }

    #[test]
    fn a_snippet_marks_what_matched() {
        let spans = |text: &str, query: &str| -> Vec<String> {
            marked(text, query)
                .into_iter()
                .map(|(text, mark)| if mark { format!("[{text}]") } else { text })
                .collect()
        };
        assert_eq!(
            spans("We ship Chat in October", "chat ship"),
            ["We ", "[ship]", " ", "[Chat]", " in October"]
        );
        assert_eq!(spans("café au lait", "CAFÉ"), ["[café]", " au lait"]);
        assert_eq!(spans("nothing here", "zebra"), ["nothing here"]);
        assert_eq!(spans("plain", ""), ["plain"]);
    }

    #[test]
    fn a_listing_opens_onto_its_files() {
        let names = "notes/\ntodo.md";
        let files = Part::Files(vec![file("/home/notes/", ""), file("/home/todo.md", "")]);
        for (args, working_dir) in [(json!({"path": "/home"}), "/else/"), (json!({}), "/home/")] {
            assert_eq!(body("list", &args, names, true, working_dir), std::slice::from_ref(&files));
        }
        assert_eq!(body("list", &json!({}), "(empty)", true, "/home/"), [line("empty")]);

        let many: Vec<String> = (0..25).map(|i| format!("{i}.md")).collect();
        let parts = body("list", &json!({}), &many.join("\n"), true, "/");
        assert!(matches!(&parts[0], Part::Files(files) if files.len() == FILES_SHOWN));
        assert_eq!(parts[1], line("5 more"));
    }

    #[test]
    fn notes_open_as_notes() {
        let note = "# Plan\n\nstep one";
        assert_eq!(
            body("read", &json!({"path": "/a.md"}), note, true, "/"),
            [Part::Note(note.into())]
        );
        assert_eq!(
            body("read", &json!({"path": "/a.txt"}), note, true, "/"),
            [Part::Text(note.into())]
        );
        let created = json!({"path": "/a.md", "text": note});
        assert_eq!(body("create", &created, "created /a.md", true, "/"), [Part::Note(note.into())]);
        assert_eq!(
            body("create", &json!({"path": "/a/"}), "created /a/", true, "/"),
            [line("created /a/")]
        );
    }

    #[test]
    fn an_edit_opens_onto_what_changed() {
        let args = json!({"path": "/n.md", "old": "a cat sat", "new": "a dog sat"});
        let diff = Part::Diff(vec![
            Change::Same("a ".into()),
            Change::Gone("cat".into()),
            Change::New("dog".into()),
            Change::Same(" sat".into()),
        ]);
        assert_eq!(
            body("edit", &args, "edited /n.md (3 replacements)", true, "/"),
            [diff.clone(), line("3 replacements")]
        );
        assert_eq!(body("edit", &args, "edited /n.md (1 replacement)", true, "/").len(), 1);
        assert_eq!(
            body("edit", &args, "old text not found", false, "/"),
            [line("old text not found"), diff]
        );
    }

    /// Every call opens onto something, even one with nothing to add.
    #[test]
    fn every_call_opens_onto_something() {
        let cases = [
            ("move", json!({"path": "/a", "to": "/b"}), "moved /a to /b", true),
            ("delete", json!({"path": "/a"}), "deleted /a", true),
            ("delete", json!({"path": "/a"}), "/a is outside this chat's folders (/home/)", false),
            ("frobnicate", json!({}), "", true),
            ("search", json!({}), "", true),
            ("read", json!({"path": "/a.md"}), "one\ntwo", false),
        ];
        for (name, args, result, ok) in cases {
            assert!(!body(name, &args, result, ok, "/").is_empty(), "{name}");
        }
        assert_eq!(
            body("delete", &json!({"path": "/a"}), "deleted /a", true, "/"),
            [line("deleted /a")]
        );
        assert_eq!(body("frobnicate", &json!({}), "", true, "/"), [line("done")]);
        assert_eq!(
            body("read", &json!({"path": "/a.md"}), "one\ntwo", false, "/"),
            [Part::Text("one\ntwo".into())]
        );
    }

    /// A card previews; the note's own tab is the full view.
    #[test]
    fn a_long_note_shows_its_start_and_how_much_is_left() {
        let lines: Vec<String> = (0..100).map(|i| format!("line {i}")).collect();
        let parts = body("read", &json!({"path": "/a.md"}), &lines.join("\n"), true, "/");
        let shown: Vec<String> = lines[..TEXT_LINES].to_vec();
        assert_eq!(parts, [Part::Note(shown.join("\n")), line("76 more lines")]);
        let short = body("read", &json!({"path": "/a.md"}), "one\ntwo", true, "/");
        assert_eq!(short, [Part::Note("one\ntwo".into())]);
    }
}
