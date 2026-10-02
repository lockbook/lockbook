//! How a tool call reads in the transcript: an icon, a verb, the path it
//! touched, and the detail shown when its row is opened.

use serde_json::Value;

use crate::style::phosphor;

pub struct Row {
    pub icon: &'static str,
    pub verb: String,
    /// Drawn in mono after the verb.
    pub path: Option<String>,
}

const DETAIL_LINES: usize = 80;
const DETAIL_CHARS: usize = 8_000;

fn arg(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|s| !s.is_empty())
}

pub fn row(name: &str, args: &Value) -> Row {
    let path = arg(args, "path");
    let row = |icon, verb: &str, path| Row { icon, verb: verb.to_string(), path };
    match name {
        "search" => {
            let query = arg(args, "query").unwrap_or_default();
            row(phosphor::SEARCH, format!("search {query}").trim_end(), None)
        }
        "read" => {
            let path = path.map(|p| match arg(args, "section") {
                Some(section) => format!("{p} § {section}"),
                None => p,
            });
            row(phosphor::FILE_TEXT, "read", path)
        }
        "list" => row(phosphor::LIST_BULLETS, "list", path),
        "edit" => row(phosphor::PENCIL, "edit", path),
        "create" => {
            let folder = path.as_deref().is_some_and(|p| p.ends_with('/'));
            let icon = if folder { phosphor::FOLDER_PLUS } else { phosphor::FILE_PLUS };
            row(icon, "create", path)
        }
        "move" => {
            let to = arg(args, "to").unwrap_or_default();
            row(phosphor::FOLDER, "move", path.map(|p| format!("{p} → {to}")))
        }
        "delete" => row(phosphor::TRASH, "delete", path),
        "request_access" => row(phosphor::LOCK_SIMPLE_OPEN, "ask for", path),
        other => row(phosphor::GEAR, other, path),
    }
}

/// Markdown for the opened row: an edit's change as a diff, a created note's
/// text, then the result, fenced unless it is one short line.
pub fn detail(name: &str, args: &Value, result: &str) -> String {
    let mut out = String::new();
    match name {
        "edit" => {
            let old = arg(args, "old").unwrap_or_default();
            let new = arg(args, "new").unwrap_or_default();
            let diff: Vec<String> = old
                .lines()
                .map(|l| format!("- {l}"))
                .chain(new.lines().map(|l| format!("+ {l}")))
                .collect();
            out.push_str(&fence("diff", &clip(&diff.join("\n"))));
        }
        "create" => {
            if let Some(text) = arg(args, "text") {
                out.push_str(&fence("", &clip(&text)));
            }
        }
        _ => {}
    }
    let result = result.trim();
    if !result.is_empty() {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        if result.contains('\n') || result.len() > 80 {
            out.push_str(&fence("", &clip(result)));
        } else {
            out.push_str(result);
        }
    }
    out
}

fn clip(text: &str) -> String {
    let mut out = String::new();
    for (i, line) in text.lines().enumerate() {
        if i >= DETAIL_LINES || out.len() >= DETAIL_CHARS {
            out.push_str("\n…");
            break;
        }
        if i > 0 {
            out.push('\n');
        }
        out.push_str(line);
    }
    out
}

/// A code fence longer than any run of backticks in `body`.
fn fence(lang: &str, body: &str) -> String {
    let longest = body.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let ticks = "`".repeat(longest.max(2) + 1);
    format!("{ticks}{lang}\n{body}\n{ticks}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn rows_read_as_verb_then_path() {
        let r = row("read", &json!({"path": "/a.md", "section": "Plan"}));
        assert_eq!((r.verb.as_str(), r.path.as_deref()), ("read", Some("/a.md § Plan")));
        let r = row("move", &json!({"path": "/a.md", "to": "/b/a.md"}));
        assert_eq!(r.path.as_deref(), Some("/a.md → /b/a.md"));
        let r = row("search", &json!({"query": "budget"}));
        assert_eq!((r.verb.as_str(), r.path), ("search budget", None));
        assert_eq!(row("create", &json!({"path": "/x/"})).icon, phosphor::FOLDER_PLUS);
        assert_eq!(row("create", &json!({"path": "/x.md"})).icon, phosphor::FILE_PLUS);
    }

    #[test]
    fn an_edit_opens_as_a_diff_then_its_result() {
        let d = detail("edit", &json!({"old": "a\nb", "new": "c"}), "edited /n.md");
        assert_eq!(d, "```diff\n- a\n- b\n+ c\n```\n\nedited /n.md");
    }

    #[test]
    fn long_or_multiline_results_are_fenced_and_clipped() {
        assert_eq!(detail("list", &json!({}), "a.md"), "a.md");
        let lines: Vec<String> = (0..100).map(|i| format!("line {i}")).collect();
        let d = detail("read", &json!({}), &lines.join("\n"));
        assert!(d.starts_with("```\nline 0\n"));
        assert!(d.ends_with("line 79\n…\n```"), "{d}");
    }

    #[test]
    fn fences_outrun_backticks_in_the_body() {
        let d = detail("read", &json!({}), "x\n```rust\ny\n```");
        assert!(d.starts_with("````\n") && d.ends_with("\n````"), "{d}");
    }
}
