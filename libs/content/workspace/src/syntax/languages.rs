use std::cell::RefCell;
use std::ops::Range;
use std::sync::OnceLock;

use tree_sitter::{Parser, Query, QueryCursor, StreamingIterator as _, Tree};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

use super::{Definition, Role, Spans};

struct Entry {
    /// What injections call it; also a fence token.
    name: &'static str,
    /// Other fence tokens, beside the name and the extensions.
    aliases: &'static [&'static str],
    extensions: &'static [&'static str],
    grammar: fn() -> tree_sitter::Language,
    /// Concatenated in order: a layered grammar lists its base's first.
    highlights: &'static [&'static str],
    injections: &'static str,
    locals: &'static str,
    /// Definitions for the outline, concatenated like the highlights.
    tags: &'static [&'static str],
}

macro_rules! entry {
    ($name:literal, [$($alias:literal),*], [$($ext:literal),*], $grammar:expr,
     [$($highlights:expr),+] $(, injections: $injections:expr)? $(, locals: $locals:expr)?
     $(, tags: [$($tags:expr),+])?) => {
        Entry {
            name: $name,
            aliases: &[$($alias),*],
            extensions: &[$($ext),*],
            grammar: || $grammar.into(),
            highlights: &[$($highlights),+],
            injections: entry!(@or $($injections)?),
            locals: entry!(@or $($locals)?),
            tags: &[$($($tags),+)?],
        }
    };
    (@or $query:expr) => { $query };
    (@or) => { "" };
}

// Data files list their sections: TOML tables, and JSON and YAML keys
// holding nested values.
const JSON_TAGS: &str =
    "(pair key: (string (string_content) @name) value: [(object) (array)]) @definition.key";
const TOML_TAGS: &str = "
(table [(bare_key) (dotted_key) (quoted_key)] @name) @definition.table
(table_array_element [(bare_key) (dotted_key) (quoted_key)] @name) @definition.table";
const YAML_TAGS: &str = "(block_mapping_pair key: (_) @name value: (block_node [(block_mapping) (block_sequence)])) @definition.key";

const ENTRIES: &[Entry] = &[
    entry!(
        "bash",
        ["sh", "shell", "zsh", "console"],
        ["sh", "bash", "zsh"],
        tree_sitter_bash::LANGUAGE,
        [tree_sitter_bash::HIGHLIGHT_QUERY]
    ),
    entry!("c", [], ["c", "h"], tree_sitter_c::LANGUAGE, [tree_sitter_c::HIGHLIGHT_QUERY], tags: [tree_sitter_c::TAGS_QUERY]),
    entry!("css", [], ["css"], tree_sitter_css::LANGUAGE, [tree_sitter_css::HIGHLIGHTS_QUERY]),
    entry!("go", ["golang"], ["go"], tree_sitter_go::LANGUAGE, [tree_sitter_go::HIGHLIGHTS_QUERY], tags: [tree_sitter_go::TAGS_QUERY]),
    entry!("html", ["htm", "xhtml"], ["html", "htm"], tree_sitter_html::LANGUAGE, [tree_sitter_html::HIGHLIGHTS_QUERY], injections: tree_sitter_html::INJECTIONS_QUERY),
    entry!("java", [], ["java"], tree_sitter_java::LANGUAGE, [tree_sitter_java::HIGHLIGHTS_QUERY], tags: [tree_sitter_java::TAGS_QUERY]),
    entry!("javascript", ["js", "node", "ecmascript"], ["js", "mjs", "cjs", "jsx"], tree_sitter_javascript::LANGUAGE,
        [tree_sitter_javascript::HIGHLIGHT_QUERY, tree_sitter_javascript::JSX_HIGHLIGHT_QUERY],
        injections: tree_sitter_javascript::INJECTIONS_QUERY, locals: tree_sitter_javascript::LOCALS_QUERY, tags: [tree_sitter_javascript::TAGS_QUERY]),
    entry!(
        "json",
        ["jsonc", "jsonl", "json5"],
        ["json", "jsonc", "jsonl"],
        tree_sitter_json::LANGUAGE,
        [tree_sitter_json::HIGHLIGHTS_QUERY], tags: [JSON_TAGS]
    ),
    entry!(
        "python",
        ["py", "py3", "python3"],
        ["py", "pyi", "pyw"],
        tree_sitter_python::LANGUAGE,
        [tree_sitter_python::HIGHLIGHTS_QUERY], tags: [tree_sitter_python::TAGS_QUERY]),
    entry!("rust", ["rs"], ["rs"], tree_sitter_rust::LANGUAGE, [tree_sitter_rust::HIGHLIGHTS_QUERY], injections: tree_sitter_rust::INJECTIONS_QUERY, tags: [tree_sitter_rust::TAGS_QUERY]),
    entry!(
        "toml",
        [],
        ["toml"],
        tree_sitter_toml_ng::LANGUAGE,
        [tree_sitter_toml_ng::HIGHLIGHTS_QUERY], tags: [TOML_TAGS]
    ),
    entry!("typescript", ["ts"], ["ts", "mts", "cts"], tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        [tree_sitter_typescript::HIGHLIGHTS_QUERY, tree_sitter_javascript::HIGHLIGHT_QUERY],
        injections: tree_sitter_javascript::INJECTIONS_QUERY, tags: [tree_sitter_javascript::TAGS_QUERY, tree_sitter_typescript::TAGS_QUERY]),
    entry!("tsx", [], ["tsx"], tree_sitter_typescript::LANGUAGE_TSX,
        [tree_sitter_typescript::HIGHLIGHTS_QUERY, tree_sitter_javascript::HIGHLIGHT_QUERY, tree_sitter_javascript::JSX_HIGHLIGHT_QUERY],
        injections: tree_sitter_javascript::INJECTIONS_QUERY, tags: [tree_sitter_javascript::TAGS_QUERY, tree_sitter_typescript::TAGS_QUERY]),
    entry!(
        "yaml",
        ["yml"],
        ["yaml", "yml"],
        tree_sitter_yaml::LANGUAGE,
        [tree_sitter_yaml::HIGHLIGHTS_QUERY], tags: [YAML_TAGS]
    ),
    #[cfg(feature = "extra-grammars")]
    entry!(
        "asm",
        ["assembly", "nasm"],
        ["asm", "s"],
        tree_sitter_asm::LANGUAGE,
        [tree_sitter_asm::HIGHLIGHTS_QUERY]
    ),
    #[cfg(feature = "extra-grammars")]
    entry!(
        "csharp",
        ["c_sharp", "cs", "c#"],
        ["cs"],
        tree_sitter_c_sharp::LANGUAGE,
        [tree_sitter_c_sharp::HIGHLIGHTS_QUERY], tags: [tree_sitter_c_sharp::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    entry!(
        "cmake",
        [],
        ["cmake"],
        tree_sitter_cmake::LANGUAGE,
        [tree_sitter_cmake::HIGHLIGHTS_QUERY]
    ),
    #[cfg(feature = "extra-grammars")]
    entry!(
        "cpp",
        ["c++", "cxx", "hpp"],
        ["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
        tree_sitter_cpp::LANGUAGE,
        [tree_sitter_c::HIGHLIGHT_QUERY, tree_sitter_cpp::HIGHLIGHT_QUERY], tags: [tree_sitter_c::TAGS_QUERY, tree_sitter_cpp::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    entry!("dart", [], ["dart"], tree_sitter_dart::LANGUAGE, [tree_sitter_dart::HIGHLIGHTS_QUERY], tags: [tree_sitter_dart::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    entry!(
        "diff",
        ["patch"],
        ["diff", "patch"],
        tree_sitter_diff::LANGUAGE,
        [tree_sitter_diff::HIGHLIGHTS_QUERY]
    ),
    #[cfg(feature = "extra-grammars")]
    entry!("elixir", ["ex", "exs"], ["ex", "exs"], tree_sitter_elixir::LANGUAGE, [tree_sitter_elixir::HIGHLIGHTS_QUERY], injections: tree_sitter_elixir::INJECTIONS_QUERY, tags: [tree_sitter_elixir::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    entry!("elm", [], ["elm"], tree_sitter_elm::LANGUAGE, [tree_sitter_elm::HIGHLIGHTS_QUERY], tags: [tree_sitter_elm::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    entry!(
        "erlang",
        ["erl"],
        ["erl", "hrl"],
        tree_sitter_erlang::LANGUAGE,
        [tree_sitter_erlang::HIGHLIGHTS_QUERY]
    ),
    #[cfg(feature = "extra-grammars")]
    entry!(
        "gleam",
        [],
        ["gleam"],
        tree_sitter_gleam::LANGUAGE,
        [tree_sitter_gleam::HIGHLIGHT_QUERY], tags: [tree_sitter_gleam::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    entry!(
        "haskell",
        ["hs"],
        ["hs"],
        tree_sitter_haskell::LANGUAGE,
        [tree_sitter_haskell::HIGHLIGHTS_QUERY]
    ),
    #[cfg(feature = "extra-grammars")]
    entry!(
        "ini",
        ["cfg", "conf"],
        ["ini", "cfg"],
        tree_sitter_ini::LANGUAGE,
        [tree_sitter_ini::HIGHLIGHTS_QUERY]
    ),
    #[cfg(feature = "extra-grammars")]
    entry!("lua", [], ["lua"], tree_sitter_lua::LANGUAGE, [tree_sitter_lua::HIGHLIGHTS_QUERY], injections: tree_sitter_lua::INJECTIONS_QUERY, tags: [tree_sitter_lua::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    entry!(
        "make",
        ["makefile", "mk"],
        ["mk", "mak"],
        tree_sitter_make::LANGUAGE,
        [tree_sitter_make::HIGHLIGHTS_QUERY]
    ),
    #[cfg(feature = "extra-grammars")]
    entry!("nix", [], ["nix"], tree_sitter_nix::LANGUAGE, [tree_sitter_nix::HIGHLIGHTS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    entry!(
        "objc",
        ["objective-c", "objectivec", "obj-c"],
        ["m", "mm"],
        tree_sitter_objc::LANGUAGE,
        [tree_sitter_c::HIGHLIGHT_QUERY, tree_sitter_objc::HIGHLIGHTS_QUERY]
    ),
    #[cfg(feature = "extra-grammars")]
    entry!("php", [], ["php"], tree_sitter_php::LANGUAGE_PHP, [tree_sitter_php::HIGHLIGHTS_QUERY], injections: tree_sitter_php::INJECTIONS_QUERY, tags: [tree_sitter_php::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    entry!(
        "powershell",
        ["ps1", "pwsh", "ps"],
        ["ps1", "psm1", "psd1"],
        tree_sitter_powershell::LANGUAGE,
        [tree_sitter_powershell::HIGHLIGHTS_QUERY]
    ),
    #[cfg(feature = "extra-grammars")]
    entry!("r", [], ["r"], tree_sitter_r::LANGUAGE, [tree_sitter_r::HIGHLIGHTS_QUERY], tags: [tree_sitter_r::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    entry!(
        "regex",
        ["regexp"],
        [],
        tree_sitter_regex::LANGUAGE,
        [tree_sitter_regex::HIGHLIGHTS_QUERY]
    ),
    #[cfg(feature = "extra-grammars")]
    entry!("ruby", ["rb"], ["rb", "rake", "gemspec"], tree_sitter_ruby::LANGUAGE, [tree_sitter_ruby::HIGHLIGHTS_QUERY], locals: tree_sitter_ruby::LOCALS_QUERY, tags: [tree_sitter_ruby::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    entry!(
        "scala",
        ["sc"],
        ["scala", "sc"],
        tree_sitter_scala::LANGUAGE,
        [tree_sitter_scala::HIGHLIGHTS_QUERY]
    ),
    #[cfg(feature = "extra-grammars")]
    entry!("swift", [], ["swift"], tree_sitter_swift::LANGUAGE, [tree_sitter_swift::HIGHLIGHTS_QUERY], injections: tree_sitter_swift::INJECTIONS_QUERY, tags: [tree_sitter_swift::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    entry!(
        "xml",
        ["svg", "plist", "xsd", "xslt"],
        ["xml", "plist", "xsd", "xsl", "xslt"],
        tree_sitter_xml::LANGUAGE_XML,
        [tree_sitter_xml::XML_HIGHLIGHT_QUERY]
    ),
    #[cfg(feature = "extra-grammars")]
    entry!("zig", [], ["zig"], tree_sitter_zig::LANGUAGE, [tree_sitter_zig::HIGHLIGHTS_QUERY], injections: tree_sitter_zig::INJECTIONS_QUERY),
];

/// Capture names we color, each with its role. A capture takes the entry
/// sharing the most dot-separated parts with it, the first on a tie, so
/// `keyword.function` is a keyword and `string.escape` a string. Plain
/// roles are listed so they cover what they nest in, like a variable
/// inside a string.
const CAPTURES: &[(&str, Role)] = &[
    ("comment", Role::Comment),
    ("keyword", Role::Keyword),
    ("conditional", Role::Keyword),
    ("repeat", Role::Keyword),
    ("include", Role::Keyword),
    ("exception", Role::Keyword),
    ("tag", Role::Keyword),
    ("string", Role::String),
    ("escape", Role::String),
    ("character", Role::String),
    ("number", Role::Number),
    ("float", Role::Number),
    ("boolean", Role::Number),
    ("constant", Role::Number),
    ("function", Role::Function),
    ("method", Role::Function),
    ("attribute", Role::Function),
    ("type", Role::Class),
    ("constructor", Role::Class),
    ("module", Role::Class),
    ("namespace", Role::Class),
    ("variable", Role::Text),
    ("property", Role::Text),
    ("field", Role::Text),
    ("parameter", Role::Text),
    ("label", Role::Text),
    ("operator", Role::Text),
    ("punctuation", Role::Text),
    ("embedded", Role::Text),
];

static CONFIGS: [OnceLock<Option<HighlightConfiguration>>; ENTRIES.len()] =
    [const { OnceLock::new() }; ENTRIES.len()];

static TAGS: [OnceLock<Option<Query>>; ENTRIES.len()] = [const { OnceLock::new() }; ENTRIES.len()];

thread_local! {
    static HIGHLIGHTER: RefCell<Highlighter> = RefCell::new(Highlighter::new());
}

pub fn find_token(token: &str) -> Option<usize> {
    let token = token.to_ascii_lowercase();
    ENTRIES.iter().position(|entry| {
        entry.name == token
            || entry.aliases.contains(&token.as_str())
            || entry.extensions.contains(&token.as_str())
    })
}

pub fn find_extension(ext: &str) -> Option<usize> {
    let ext = ext.to_ascii_lowercase();
    ENTRIES
        .iter()
        .position(|entry| entry.extensions.contains(&ext.as_str()))
}

/// Language names a fence's info could be completing `query` toward: those
/// whose name, then an alias, starts with it, shortest first.
pub fn complete(query: &str) -> Vec<&'static str> {
    let query = query.to_ascii_lowercase();
    let mut found: Vec<_> = ENTRIES
        .iter()
        .filter_map(|entry| {
            let tier = if entry.name.starts_with(&query) {
                0
            } else if entry
                .aliases
                .iter()
                .chain(entry.extensions)
                .any(|a| a.starts_with(&query))
            {
                1
            } else {
                return None;
            };
            Some((tier, entry.name.len(), entry.name))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, _, name)| name).collect()
}

pub fn name(idx: usize) -> &'static str {
    ENTRIES[idx].name
}

fn config(idx: usize) -> Option<&'static HighlightConfiguration> {
    CONFIGS[idx]
        .get_or_init(|| {
            let entry = &ENTRIES[idx];
            let highlights = entry.highlights.concat();
            let config = HighlightConfiguration::new(
                (entry.grammar)(),
                entry.name,
                &highlights,
                entry.injections,
                entry.locals,
            );
            match config {
                Ok(mut config) => {
                    let names: Vec<_> = CAPTURES.iter().map(|(name, _)| *name).collect();
                    config.configure(&names);
                    Some(config)
                }
                Err(err) => {
                    tracing::error!("{} highlight queries don't compile: {err:?}", entry.name);
                    None
                }
            }
        })
        .as_ref()
}

pub fn highlight(idx: usize, text: &str) -> Spans {
    let Some(unit) = config(idx) else { return Vec::new() };
    HIGHLIGHTER.with_borrow_mut(|highlighter| {
        let injected = |name: &str| find_token(name).and_then(config);
        let Ok(events) = highlighter.highlight(unit, text.as_bytes(), None, None, injected) else {
            return Vec::new();
        };
        let mut spans: Spans = Vec::new();
        let mut roles = Vec::new();
        for event in events {
            match event {
                Ok(HighlightEvent::HighlightStart(capture)) => roles.push(CAPTURES[capture.0].1),
                Ok(HighlightEvent::HighlightEnd) => {
                    roles.pop();
                }
                Ok(HighlightEvent::Source { start, end }) => {
                    let role = roles.last().copied().unwrap_or(Role::Text);
                    if role == Role::Text || start == end {
                        continue;
                    }
                    match spans.last_mut() {
                        Some((last, range)) if *last == role && range.end == start => {
                            range.end = end
                        }
                        _ => spans.push((role, start..end)),
                    }
                }
                Err(_) => break,
            }
        }
        spans
    })
}

fn parse(idx: usize, text: &str) -> Option<Tree> {
    let mut parser = Parser::new();
    parser.set_language(&(ENTRIES[idx].grammar)()).ok()?;
    parser.parse(text, None)
}

fn tags(idx: usize) -> Option<&'static Query> {
    TAGS[idx]
        .get_or_init(|| {
            let entry = &ENTRIES[idx];
            if entry.tags.is_empty() {
                return None;
            }
            match Query::new(&(entry.grammar)(), &entry.tags.concat()) {
                Ok(query) => Some(query),
                Err(err) => {
                    tracing::error!("{} tags query doesn't compile: {err:?}", entry.name);
                    None
                }
            }
        })
        .as_ref()
}

pub fn has_definitions(idx: usize) -> bool {
    !ENTRIES[idx].tags.is_empty()
}

pub fn definitions(idx: usize, text: &str) -> Vec<Definition> {
    let (Some(query), Some(tree)) = (tags(idx), parse(idx, text)) else { return Vec::new() };
    let names = query.capture_names();
    let mut found: Vec<(Range<usize>, Range<usize>)> = Vec::new();
    let mut cursor = QueryCursor::new();
    let mut matches = cursor.matches(query, tree.root_node(), text.as_bytes());
    while let Some(m) = matches.next() {
        let capture = |pick: &dyn Fn(&str) -> bool| {
            m.captures()
                .iter()
                .find(|c| pick(names[c.index as usize]))
                .map(|c| c.node.byte_range())
        };
        let name = capture(&|name| name == "name");
        let item = capture(&|name| name.starts_with("definition."));
        if let (Some(name), Some(item)) = (name, item) {
            found.push((name, item));
        }
    }
    found.sort_by_key(|(name, item)| (item.start, std::cmp::Reverse(item.end), name.start));
    found.dedup_by_key(|(name, _)| name.clone());
    let mut within: Vec<Range<usize>> = Vec::new();
    found
        .into_iter()
        .map(|(name, item)| {
            while within.last().is_some_and(|outer| outer.end <= item.start) {
                within.pop();
            }
            let depth = within.len();
            within.push(item.clone());
            Definition { start: item.start, name, depth }
        })
        .collect()
}

pub fn enclosing(idx: usize, text: &str, range: Range<usize>) -> Option<Range<usize>> {
    let tree = parse(idx, text)?;
    let root = tree.root_node();
    let mut node = if range.is_empty() {
        // a caret is in the token on either side of it: the shorter, a named
        // one on a tie, else the one before
        let at = range.start;
        let before = at
            .checked_sub(1)
            .and_then(|b| root.descendant_for_byte_range(b, at));
        let after = root.descendant_for_byte_range(at, at + 1);
        let key = |node: &tree_sitter::Node| (node.byte_range().len(), !node.is_named());
        match (before, after) {
            (Some(before), Some(after)) if key(&after) < key(&before) => after,
            (Some(before), _) => before,
            (None, after) => after?,
        }
    } else {
        root.descendant_for_byte_range(range.start, range.end)?
    };
    loop {
        let found = node.byte_range();
        if found.start <= range.start && range.end <= found.end && found != range {
            return Some(found);
        }
        node = node.parent()?;
    }
}

#[cfg(test)]
mod test {
    use super::{CONFIGS, ENTRIES, config, definitions, enclosing, find_token, highlight, tags};
    use crate::syntax::Role;

    #[test]
    fn every_language_compiles() {
        let broken: Vec<_> = (0..ENTRIES.len())
            .filter(|idx| config(*idx).is_none())
            .map(|idx| ENTRIES[idx].name)
            .collect();
        assert!(broken.is_empty(), "queries don't compile: {broken:?}");
        let broken: Vec<_> = (0..ENTRIES.len())
            .filter(|idx| !ENTRIES[*idx].tags.is_empty() && tags(*idx).is_none())
            .map(|idx| ENTRIES[idx].name)
            .collect();
        assert!(broken.is_empty(), "tags queries don't compile: {broken:?}");
        assert_eq!(CONFIGS.len(), ENTRIES.len());
    }

    #[test]
    fn comments_span_lines() {
        let rust = find_token("rust").unwrap();
        let text = "/* one\ntwo */ fn";
        let spans = highlight(rust, text);
        assert_eq!(spans[0], (Role::Comment, 0..13));
    }

    #[test]
    fn definitions_nest() {
        let python = find_token("python").unwrap();
        let text = "class A:\n    def b(self):\n        pass\n\ndef c():\n    pass\n";
        let found: Vec<_> = definitions(python, text)
            .into_iter()
            .map(|d| (&text[d.name], d.depth))
            .collect();
        assert_eq!(found, vec![("A", 0), ("b", 1), ("c", 0)]);
    }

    #[test]
    fn data_files_list_sections() {
        let names = |lang: &str, text: &str| -> Vec<(String, usize)> {
            definitions(find_token(lang).unwrap(), text)
                .into_iter()
                .map(|d| (text[d.name].to_string(), d.depth))
                .collect()
        };
        let toml = "[package]\nname = \"x\"\n\n[[bin]]\npath = \"a\"\n[a.b]\n";
        let want = [("package", 0), ("bin", 0), ("a.b", 0)];
        assert_eq!(names("toml", toml), want.map(|(n, d)| (n.into(), d)));
        let json = r#"{"a": {"b": [1], "c": 2}, "d": 3}"#;
        assert_eq!(names("json", json), vec![("a".into(), 0), ("b".into(), 1)]);
        let yaml = "a:\n  b:\n    - 1\n  c: 2\nd: |\n  text\n";
        assert_eq!(names("yaml", yaml), vec![("a".into(), 0), ("b".into(), 1)]);
    }

    #[test]
    fn enclosing_grows() {
        let rust = find_token("rust").unwrap();
        let text = "fn f() { g(1, 2); }";
        let one = text.find('1').unwrap();
        let args = enclosing(rust, text, one..one + 1).unwrap();
        assert_eq!(&text[args.clone()], "(1, 2)");
        let call = enclosing(rust, text, args).unwrap();
        assert_eq!(&text[call], "g(1, 2)");
    }
}
