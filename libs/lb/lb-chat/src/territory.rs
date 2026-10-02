//! Where a chat's agent may roam: an include list minus an exclude list,
//! exclude winning at any depth. Dotted names are always out, and so is
//! anything a `.agentignore` file walls off below the folder it sits in. A
//! walled path does not exist as far as the model is concerned, and neither
//! does the chat's own file.

use lb_rs::blocking::Lb;
use lb_rs::model::chat::Settings;

pub const IGNORE_FILE: &str = ".agentignore";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Territory {
    /// The chat's folder, with a trailing slash. Always included.
    pub working_dir: String,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    /// `(folder with a trailing slash, patterns relative to it)`.
    pub ignores: Vec<(String, Vec<String>)>,
    /// The chat's own file, which its tools never see.
    pub own: Option<String>,
}

impl Territory {
    pub fn new(working_dir: &str, settings: &Settings) -> Self {
        Self {
            working_dir: folder(working_dir),
            include: settings.include.iter().map(|p| normalize(p)).collect(),
            exclude: settings.exclude.iter().map(|p| normalize(p)).collect(),
            ignores: Vec::new(),
            own: None,
        }
    }

    /// Reads every `.agentignore` in the vault.
    pub fn load(lb: &Lb, working_dir: &str, settings: &Settings) -> Self {
        let mut territory = Self::new(working_dir, settings);
        for file in lb.list_metadatas().unwrap_or_default() {
            if file.name != IGNORE_FILE || !file.is_document() {
                continue;
            }
            let (Ok(path), Ok(bytes)) =
                (lb.get_path_by_id(file.id), lb.read_document(file.id, false))
            else {
                continue;
            };
            let dir = folder(&path[..path.len() - IGNORE_FILE.len()]);
            territory
                .ignores
                .push((dir, parse_ignore(&String::from_utf8_lossy(&bytes))));
        }
        territory
    }

    /// The model may see and touch `path`.
    pub fn visible(&self, path: &str) -> bool {
        self.allowed(path) && !self.walled(path) && !self.own(path)
    }

    /// `path` is the chat's own file.
    pub fn own(&self, path: &str) -> bool {
        let path = normalize(path);
        self.own.as_deref() == Some(path.trim_end_matches('/'))
    }

    /// `path` is inside the include set.
    pub fn allowed(&self, path: &str) -> bool {
        let path = normalize(path);
        under(&path, &self.working_dir) || self.include.iter().any(|root| under(&path, root))
    }

    /// `path` is behind a wall no chat can lift: dotted, excluded, or ignored.
    pub fn walled(&self, path: &str) -> bool {
        let path = normalize(path);
        if path.split('/').any(|seg| seg.starts_with('.')) {
            return true;
        }
        if self.exclude.iter().any(|root| under(&path, root)) {
            return true;
        }
        self.ignores.iter().any(|(dir, patterns)| {
            path.strip_prefix(dir.as_str())
                .is_some_and(|rest| patterns.iter().any(|p| pattern_matches(p, rest)))
        })
    }

    /// The roots the prompt names, working directory first.
    pub fn roots(&self) -> Vec<String> {
        let mut roots = vec![self.working_dir.clone()];
        roots.extend(
            self.include
                .iter()
                .filter(|r| !under(r, &self.working_dir))
                .cloned(),
        );
        roots
    }
}

/// Collapses repeated slashes and resolves `.` and `..` segments.
pub fn normalize(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            s => out.push(s),
        }
    }
    let mut s = format!("/{}", out.join("/"));
    if path.ends_with('/') && s.len() > 1 {
        s.push('/');
    }
    s
}

fn folder(path: &str) -> String {
    let mut s = normalize(path);
    if !s.ends_with('/') {
        s.push('/');
    }
    s
}

/// `root` ending in `/` covers itself and everything below; otherwise it
/// names one file.
fn under(path: &str, root: &str) -> bool {
    if let Some(dir) = root.strip_suffix('/') {
        path == dir || path == root || path.starts_with(root)
    } else {
        path == root || path.strip_suffix('/') == Some(root)
    }
}

fn parse_ignore(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(|l| l.trim_start_matches('/').to_string())
        .collect()
}

/// A pattern's segments must match the leading segments of `rest`; `*`
/// matches within a segment.
fn pattern_matches(pattern: &str, rest: &str) -> bool {
    let pattern: Vec<&str> = pattern.split('/').filter(|s| !s.is_empty()).collect();
    let rest: Vec<&str> = rest.split('/').filter(|s| !s.is_empty()).collect();
    pattern.len() <= rest.len() && pattern.iter().zip(&rest).all(|(p, r)| glob(p, r))
}

fn glob(pattern: &str, text: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == text,
        Some((head, tail)) => text.strip_prefix(head).is_some_and(|after| {
            (0..=after.len())
                .filter(|&i| after.is_char_boundary(i))
                .any(|i| glob(tail, &after[i..]))
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn territory() -> Territory {
        let settings = Settings {
            model: None,
            include: vec!["/team/".into(), "/single.md".into()],
            exclude: vec!["/team/drafts/".into()],
            effort: None,
        };
        let mut t = Territory::new("/home/", &settings);
        t.ignores.push((
            "/team/".into(),
            parse_ignore("# secrets\nprod-secrets/\n*.key\nnotes/private*.md\n"),
        ));
        t
    }

    #[test]
    fn working_dir_and_includes_are_allowed_everything_else_is_not() {
        let t = territory();
        assert!(t.visible("/home/todo.md"));
        assert!(t.visible("/home/deep/er/note.md"));
        assert!(t.visible("/team/plan.md"));
        assert!(t.visible("/single.md"));
        assert!(!t.visible("/elsewhere/note.md"));
        assert!(!t.visible("/"));
        assert!(!t.walled("/elsewhere/note.md"));
    }

    #[test]
    fn walls_beat_grants_at_any_depth() {
        let t = territory();
        assert!(t.walled("/team/drafts/x.md"));
        assert!(t.walled("/team/prod-secrets/keys.md"));
        assert!(t.walled("/team/prod-secrets/"));
        assert!(t.walled("/team/api.key"));
        assert!(t.walled("/team/notes/private-journal.md"));
        assert!(!t.walled("/team/notes/public.md"));
        assert!(t.walled("/home/.agent/providers/x.json"));
        assert!(t.walled("/home/.agentignore"));
        assert!(!t.visible("/team/drafts/x.md"));
    }

    #[test]
    fn the_chats_own_file_is_out_and_other_chats_are_not() {
        let mut t = territory();
        t.own = Some("/home/talk.chat".into());
        assert!(!t.visible("/home/talk.chat"));
        assert!(!t.visible("/home//talk.chat/"));
        assert!(!t.walled("/home/talk.chat"));
        assert!(t.visible("/home/other.chat"));
        assert!(t.visible("/team/talk.chat"));
    }

    #[test]
    fn ignore_files_only_cover_their_own_folder() {
        let t = territory();
        assert!(!t.walled("/home/prod-secrets/keys.md"));
    }

    #[test]
    fn paths_normalize_before_checks() {
        let t = territory();
        assert!(t.visible("/home//todo.md"));
        assert!(!t.visible("/home/../elsewhere/note.md"));
        assert!(t.walled("/team/x/../prod-secrets/k.md"));
        assert_eq!(normalize("/a/b/../c/"), "/a/c/");
        assert_eq!(normalize("a//b"), "/a/b");
    }

    #[test]
    fn globs_match_within_a_segment() {
        assert!(glob("*.key", "api.key"));
        assert!(glob("private*", "private-journal.md"));
        assert!(!glob("*.key", "api.keys"));
        assert!(glob("a*b*c", "axxbyyc"));
        assert!(!glob("a*b", "ab/c"));
    }

    #[test]
    fn roots_name_the_working_dir_first() {
        assert_eq!(territory().roots(), ["/home/", "/team/", "/single.md"]);
    }
}
