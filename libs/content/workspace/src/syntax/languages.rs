use std::ops::Range;
use std::sync::OnceLock;

use tree_sitter::{Node, Query, QueryCursor, QueryMatch, StreamingIterator as _, Tree};
use tree_sitter_language::LanguageFn;

use super::{Definition, Role};

struct Entry {
    /// What injections call it; also a fence token.
    name: &'static str,
    /// Other fence tokens, beside the name and the extensions.
    aliases: &'static [&'static str],
    extensions: &'static [&'static str],
    grammar: LanguageFn,
    /// Concatenated in order: a layered grammar lists its base's first.
    highlights: &'static [&'static str],
    injections: &'static str,
    /// Definitions for the outline, concatenated like the highlights.
    tags: &'static [&'static str],
}

impl Entry {
    const fn new(
        name: &'static str, aliases: &'static [&'static str], extensions: &'static [&'static str],
        grammar: LanguageFn, highlights: &'static [&'static str],
    ) -> Self {
        Self { name, aliases, extensions, grammar, highlights, injections: "", tags: &[] }
    }

    const fn injections(self, injections: &'static str) -> Self {
        Self { injections, ..self }
    }

    const fn tags(self, tags: &'static [&'static str]) -> Self {
        Self { tags, ..self }
    }
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
    Entry::new(
        "bash",
        &["sh", "shell", "zsh", "console"],
        &["sh", "bash", "zsh"],
        tree_sitter_bash::LANGUAGE,
        &[tree_sitter_bash::HIGHLIGHT_QUERY],
    ),
    Entry::new("c", &[], &["c", "h"], tree_sitter_c::LANGUAGE, &[tree_sitter_c::HIGHLIGHT_QUERY])
        .tags(&[tree_sitter_c::TAGS_QUERY]),
    Entry::new(
        "css",
        &[],
        &["css"],
        tree_sitter_css::LANGUAGE,
        &[tree_sitter_css::HIGHLIGHTS_QUERY],
    ),
    Entry::new(
        "go",
        &["golang"],
        &["go"],
        tree_sitter_go::LANGUAGE,
        &[tree_sitter_go::HIGHLIGHTS_QUERY],
    )
    .tags(&[tree_sitter_go::TAGS_QUERY]),
    Entry::new(
        "html",
        &["htm", "xhtml"],
        &["html", "htm"],
        tree_sitter_html::LANGUAGE,
        &[tree_sitter_html::HIGHLIGHTS_QUERY],
    )
    .injections(tree_sitter_html::INJECTIONS_QUERY),
    Entry::new(
        "java",
        &[],
        &["java"],
        tree_sitter_java::LANGUAGE,
        &[tree_sitter_java::HIGHLIGHTS_QUERY],
    )
    .tags(&[tree_sitter_java::TAGS_QUERY]),
    Entry::new(
        "javascript",
        &["js", "node", "ecmascript"],
        &["js", "mjs", "cjs", "jsx"],
        tree_sitter_javascript::LANGUAGE,
        &[tree_sitter_javascript::HIGHLIGHT_QUERY, tree_sitter_javascript::JSX_HIGHLIGHT_QUERY],
    )
    .injections(tree_sitter_javascript::INJECTIONS_QUERY)
    .tags(&[tree_sitter_javascript::TAGS_QUERY]),
    Entry::new(
        "json",
        &["jsonc", "jsonl", "json5"],
        &["json", "jsonc", "jsonl"],
        tree_sitter_json::LANGUAGE,
        &[tree_sitter_json::HIGHLIGHTS_QUERY],
    )
    .tags(&[JSON_TAGS]),
    Entry::new(
        "python",
        &["py", "py3", "python3"],
        &["py", "pyi", "pyw"],
        tree_sitter_python::LANGUAGE,
        &[tree_sitter_python::HIGHLIGHTS_QUERY],
    )
    .tags(&[tree_sitter_python::TAGS_QUERY]),
    Entry::new(
        "rust",
        &["rs"],
        &["rs"],
        tree_sitter_rust::LANGUAGE,
        &[tree_sitter_rust::HIGHLIGHTS_QUERY],
    )
    .injections(tree_sitter_rust::INJECTIONS_QUERY)
    .tags(&[tree_sitter_rust::TAGS_QUERY]),
    Entry::new(
        "toml",
        &[],
        &["toml"],
        tree_sitter_toml_ng::LANGUAGE,
        &[tree_sitter_toml_ng::HIGHLIGHTS_QUERY],
    )
    .tags(&[TOML_TAGS]),
    Entry::new(
        "typescript",
        &["ts"],
        &["ts", "mts", "cts"],
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT,
        &[tree_sitter_typescript::HIGHLIGHTS_QUERY, tree_sitter_javascript::HIGHLIGHT_QUERY],
    )
    .injections(tree_sitter_javascript::INJECTIONS_QUERY)
    .tags(&[tree_sitter_javascript::TAGS_QUERY, tree_sitter_typescript::TAGS_QUERY]),
    Entry::new(
        "tsx",
        &[],
        &["tsx"],
        tree_sitter_typescript::LANGUAGE_TSX,
        &[
            tree_sitter_typescript::HIGHLIGHTS_QUERY,
            tree_sitter_javascript::HIGHLIGHT_QUERY,
            tree_sitter_javascript::JSX_HIGHLIGHT_QUERY,
        ],
    )
    .injections(tree_sitter_javascript::INJECTIONS_QUERY)
    .tags(&[tree_sitter_javascript::TAGS_QUERY, tree_sitter_typescript::TAGS_QUERY]),
    Entry::new(
        "yaml",
        &["yml"],
        &["yaml", "yml"],
        tree_sitter_yaml::LANGUAGE,
        &[tree_sitter_yaml::HIGHLIGHTS_QUERY],
    )
    .tags(&[YAML_TAGS]),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "asm",
        &["assembly", "nasm"],
        &["asm", "s"],
        tree_sitter_asm::LANGUAGE,
        &[tree_sitter_asm::HIGHLIGHTS_QUERY],
    ),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "csharp",
        &["c_sharp", "cs", "c#"],
        &["cs"],
        tree_sitter_c_sharp::LANGUAGE,
        &[tree_sitter_c_sharp::HIGHLIGHTS_QUERY],
    )
    .tags(&[tree_sitter_c_sharp::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "cmake",
        &[],
        &["cmake"],
        tree_sitter_cmake::LANGUAGE,
        &[tree_sitter_cmake::HIGHLIGHTS_QUERY],
    ),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "cpp",
        &["c++", "cxx", "hpp"],
        &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
        tree_sitter_cpp::LANGUAGE,
        &[tree_sitter_c::HIGHLIGHT_QUERY, tree_sitter_cpp::HIGHLIGHT_QUERY],
    )
    .tags(&[tree_sitter_c::TAGS_QUERY, tree_sitter_cpp::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "dart",
        &[],
        &["dart"],
        tree_sitter_dart::LANGUAGE,
        &[tree_sitter_dart::HIGHLIGHTS_QUERY],
    )
    .tags(&[tree_sitter_dart::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "diff",
        &["patch"],
        &["diff", "patch"],
        tree_sitter_diff::LANGUAGE,
        &[tree_sitter_diff::HIGHLIGHTS_QUERY],
    ),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "elixir",
        &["ex", "exs"],
        &["ex", "exs"],
        tree_sitter_elixir::LANGUAGE,
        &[tree_sitter_elixir::HIGHLIGHTS_QUERY],
    )
    .injections(tree_sitter_elixir::INJECTIONS_QUERY)
    .tags(&[tree_sitter_elixir::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "elm",
        &[],
        &["elm"],
        tree_sitter_elm::LANGUAGE,
        &[tree_sitter_elm::HIGHLIGHTS_QUERY],
    )
    .tags(&[tree_sitter_elm::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "erlang",
        &["erl"],
        &["erl", "hrl"],
        tree_sitter_erlang::LANGUAGE,
        &[tree_sitter_erlang::HIGHLIGHTS_QUERY],
    ),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "gleam",
        &[],
        &["gleam"],
        tree_sitter_gleam::LANGUAGE,
        &[tree_sitter_gleam::HIGHLIGHT_QUERY],
    )
    .tags(&[tree_sitter_gleam::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "haskell",
        &["hs"],
        &["hs"],
        tree_sitter_haskell::LANGUAGE,
        &[tree_sitter_haskell::HIGHLIGHTS_QUERY],
    ),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "ini",
        &["cfg", "conf"],
        &["ini", "cfg"],
        tree_sitter_ini::LANGUAGE,
        &[tree_sitter_ini::HIGHLIGHTS_QUERY],
    ),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "lua",
        &[],
        &["lua"],
        tree_sitter_lua::LANGUAGE,
        &[tree_sitter_lua::HIGHLIGHTS_QUERY],
    )
    .injections(tree_sitter_lua::INJECTIONS_QUERY)
    .tags(&[tree_sitter_lua::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "make",
        &["makefile", "mk"],
        &["mk", "mak"],
        tree_sitter_make::LANGUAGE,
        &[tree_sitter_make::HIGHLIGHTS_QUERY],
    ),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "nix",
        &[],
        &["nix"],
        tree_sitter_nix::LANGUAGE,
        &[tree_sitter_nix::HIGHLIGHTS_QUERY],
    ),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "objc",
        &["objective-c", "objectivec", "obj-c"],
        &["m", "mm"],
        tree_sitter_objc::LANGUAGE,
        &[tree_sitter_c::HIGHLIGHT_QUERY, tree_sitter_objc::HIGHLIGHTS_QUERY],
    ),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "php",
        &[],
        &["php"],
        tree_sitter_php::LANGUAGE_PHP,
        &[tree_sitter_php::HIGHLIGHTS_QUERY],
    )
    .injections(tree_sitter_php::INJECTIONS_QUERY)
    .tags(&[tree_sitter_php::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "powershell",
        &["ps1", "pwsh", "ps"],
        &["ps1", "psm1", "psd1"],
        tree_sitter_powershell::LANGUAGE,
        &[tree_sitter_powershell::HIGHLIGHTS_QUERY],
    ),
    #[cfg(feature = "extra-grammars")]
    Entry::new("r", &[], &["r"], tree_sitter_r::LANGUAGE, &[tree_sitter_r::HIGHLIGHTS_QUERY])
        .tags(&[tree_sitter_r::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "regex",
        &["regexp"],
        &[],
        tree_sitter_regex::LANGUAGE,
        &[tree_sitter_regex::HIGHLIGHTS_QUERY],
    ),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "ruby",
        &["rb"],
        &["rb", "rake", "gemspec"],
        tree_sitter_ruby::LANGUAGE,
        &[tree_sitter_ruby::HIGHLIGHTS_QUERY],
    )
    .tags(&[tree_sitter_ruby::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "scala",
        &["sc"],
        &["scala", "sc"],
        tree_sitter_scala::LANGUAGE,
        &[tree_sitter_scala::HIGHLIGHTS_QUERY],
    ),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "swift",
        &[],
        &["swift"],
        tree_sitter_swift::LANGUAGE,
        &[tree_sitter_swift::HIGHLIGHTS_QUERY],
    )
    .injections(tree_sitter_swift::INJECTIONS_QUERY)
    .tags(&[tree_sitter_swift::TAGS_QUERY]),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "xml",
        &["svg", "plist", "xsd", "xslt"],
        &["xml", "plist", "xsd", "xsl", "xslt"],
        tree_sitter_xml::LANGUAGE_XML,
        &[tree_sitter_xml::XML_HIGHLIGHT_QUERY],
    ),
    #[cfg(feature = "extra-grammars")]
    Entry::new(
        "zig",
        &[],
        &["zig"],
        tree_sitter_zig::LANGUAGE,
        &[tree_sitter_zig::HIGHLIGHTS_QUERY],
    )
    .injections(tree_sitter_zig::INJECTIONS_QUERY),
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
    ("special", Role::Comment),
    ("variable", Role::Text),
    ("property", Role::Text),
    ("field", Role::Text),
    ("parameter", Role::Text),
    ("label", Role::Text),
    ("operator", Role::Text),
    ("punctuation", Role::Text),
    ("embedded", Role::Text),
];

static QUERIES: [OnceLock<Option<Queries>>; ENTRIES.len()] =
    [const { OnceLock::new() }; ENTRIES.len()];

static TAGS: [OnceLock<Option<Query>>; ENTRIES.len()] = [const { OnceLock::new() }; ENTRIES.len()];

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

pub fn grammar(idx: usize) -> tree_sitter::Language {
    tree_sitter::Language::new(ENTRIES[idx].grammar)
}

/// A language's queries, compiled once.
pub struct Queries {
    pub highlights: Query,
    /// The role each highlights capture colors, by capture index.
    pub roles: Vec<Option<Role>>,
    /// Injections that are each a layer of their own.
    pub injections: Option<Query>,
    /// Injections whose matches share one layer per pattern.
    pub combined: Option<Query>,
}

/// One match of an injections query: the language it names, if any, and
/// the node holding its code.
pub struct Injection<'tree, 'text> {
    pub language: Option<&'text str>,
    pub content: Node<'tree>,
    pub include_children: bool,
}

pub fn queries(idx: usize) -> Option<&'static Queries> {
    QUERIES[idx]
        .get_or_init(|| {
            let entry = &ENTRIES[idx];
            let compiled = (|| {
                let highlights = Query::new(&grammar(idx), &entry.highlights.concat())?;
                let roles = highlights
                    .capture_names()
                    .iter()
                    .map(|name| role(name))
                    .collect();
                let (injections, combined) = injection_queries(idx)?;
                Ok::<_, tree_sitter::QueryError>(Queries {
                    highlights,
                    roles,
                    injections,
                    combined,
                })
            })();
            compiled
                .inspect_err(|err| tracing::error!("{} queries don't compile: {err:?}", entry.name))
                .ok()
        })
        .as_ref()
}

/// The role of a capture: that of the first entry in [`CAPTURES`] it
/// names a part of.
fn role(capture: &str) -> Option<Role> {
    let parts: Vec<_> = capture.split('.').collect();
    CAPTURES
        .iter()
        .find(|(name, _)| parts.contains(name))
        .map(|(_, role)| *role)
}

/// The injections query split in two: patterns that are each their own
/// layer, and patterns marked `injection.combined`.
#[allow(clippy::type_complexity)]
fn injection_queries(
    idx: usize,
) -> Result<(Option<Query>, Option<Query>), tree_sitter::QueryError> {
    let source = ENTRIES[idx].injections;
    if source.is_empty() {
        return Ok((None, None));
    }
    let mut separate = Query::new(&grammar(idx), source)?;
    let mut combined = Query::new(&grammar(idx), source)?;
    let mut any_combined = false;
    for pattern in 0..separate.pattern_count() {
        let is_combined = separate
            .property_settings(pattern)
            .iter()
            .any(|setting| &*setting.key == "injection.combined");
        if is_combined {
            any_combined = true;
            separate.disable_pattern(pattern);
        } else {
            combined.disable_pattern(pattern);
        }
    }
    Ok((Some(separate), any_combined.then_some(combined)))
}

/// What `m`, a match of an injections `query`, injects: the language a
/// capture's text or a `#set!` names, and its content, if it has one.
pub fn injection<'tree, 'text>(
    query: &'text Query, m: &QueryMatch<'_, 'tree>, text: &'text str,
) -> Option<Injection<'tree, 'text>> {
    let names = query.capture_names();
    let mut lang = None;
    let mut content = None;
    for capture in m.captures() {
        match names[capture.index as usize] {
            "injection.language" => lang = capture.node.utf8_text(text.as_bytes()).ok(),
            "injection.content" => content = Some(capture.node),
            _ => {}
        }
    }
    let mut include_children = false;
    for setting in query.property_settings(m.pattern_index) {
        match &*setting.key {
            "injection.language" if lang.is_none() => lang = setting.value.as_deref(),
            "injection.include-children" => include_children = true,
            _ => {}
        }
    }
    Some(Injection { language: lang, content: content?, include_children })
}

fn tags(idx: usize) -> Option<&'static Query> {
    TAGS[idx]
        .get_or_init(|| {
            let entry = &ENTRIES[idx];
            if entry.tags.is_empty() {
                return None;
            }
            match Query::new(&grammar(idx), &entry.tags.concat()) {
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

pub fn definitions(idx: usize, tree: &Tree, text: &str) -> Vec<Definition> {
    let Some(query) = tags(idx) else { return Vec::new() };
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

pub fn enclosing(tree: &Tree, range: Range<usize>) -> Option<Range<usize>> {
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
    use super::{ENTRIES, QUERIES, definitions, enclosing, find_token, queries, tags};
    use crate::syntax::highlight::Highlights;
    use crate::syntax::{Role, Spans};

    fn highlight(idx: usize, text: &str) -> Spans {
        let highlights = Highlights::new(idx, text.into()).unwrap();
        highlights.spans(0..text.len()).as_ref().clone()
    }

    fn parsed(lang: &str, text: &str) -> (usize, Highlights) {
        let idx = find_token(lang).unwrap();
        (idx, Highlights::new(idx, text.into()).unwrap())
    }

    #[test]
    fn every_language_compiles() {
        let broken: Vec<_> = (0..ENTRIES.len())
            .filter(|idx| queries(*idx).is_none())
            .map(|idx| ENTRIES[idx].name)
            .collect();
        assert!(broken.is_empty(), "queries don't compile: {broken:?}");
        let broken: Vec<_> = (0..ENTRIES.len())
            .filter(|idx| !ENTRIES[*idx].tags.is_empty() && tags(*idx).is_none())
            .map(|idx| ENTRIES[idx].name)
            .collect();
        assert!(broken.is_empty(), "tags queries don't compile: {broken:?}");
        assert_eq!(QUERIES.len(), ENTRIES.len());
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
        let text = "class A:\n    def b(self):\n        pass\n\ndef c():\n    pass\n";
        let (python, parsed) = parsed("python", text);
        let found: Vec<_> = definitions(python, parsed.tree(), text)
            .into_iter()
            .map(|d| (&text[d.name], d.depth))
            .collect();
        assert_eq!(found, vec![("A", 0), ("b", 1), ("c", 0)]);
    }

    #[test]
    fn data_files_list_sections() {
        let names = |lang: &str, text: &str| -> Vec<(String, usize)> {
            let (lang, parsed) = parsed(lang, text);
            definitions(lang, parsed.tree(), text)
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
        let text = "fn f() { g(1, 2); }";
        let (_, parsed) = parsed("rust", text);
        let one = text.find('1').unwrap();
        let args = enclosing(parsed.tree(), one..one + 1).unwrap();
        assert_eq!(&text[args.clone()], "(1, 2)");
        let call = enclosing(parsed.tree(), args).unwrap();
        assert_eq!(&text[call], "g(1, 2)");
    }
}
