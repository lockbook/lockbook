//! The links in a note's text, read as the editor reads them: what each one
//! is written with, and the words around it.

use std::ops::Range;
use std::sync::Arc;

use comrak::Arena;
use comrak::nodes::{AstNode, LineColumn, NodeValue};

use crate::file_cache::{link_id, web_link};
use crate::front_matter::{self, url_value, yaml_scalar};
use crate::tab::markdown_editor::MdRender;
use crate::tab::markdown_editor::fragment::{Heading, headings_in};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum LinkKind {
    /// `[label](destination)`, `<destination>`, or a bare URL.
    Link,
    /// `![alt](destination)`.
    Image,
    /// `[[title]]` or `[[title|label]]`.
    Wiki,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoteLink {
    pub kind: LinkKind,
    /// A URL or path, or a wiki title, with any `#heading`.
    pub dest: String,
    /// Where the link sits in the note, in bytes.
    pub span: Range<usize>,
    /// The words around the link, as plain text, and where its own are.
    pub context: String,
    pub label: Range<usize>,
    /// Whether the destination is written inside the link as it reads, so
    /// it can be rewritten: not a reference link's, nor one spelled with
    /// escapes or entities.
    pub rewritable: bool,
}

/// What the index keeps of a note.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NoteText {
    pub headings: Arc<Vec<Heading>>,
    pub links: Vec<NoteLink>,
    /// The front matter's properties: each key with its value, or its
    /// items. A scalar longer than [`PROPERTY_VALUE_BYTES`] is left out.
    pub properties: Vec<(String, Vec<String>)>,
}

/// The longest scalar kept as a property value; a paragraph is not a
/// completion.
pub const PROPERTY_VALUE_BYTES: usize = 80;

/// Plain text kept on either side of a link's own words.
const CONTEXT_BYTES: usize = 160;

/// Reads a note's headings and the links that may reach a file, those in
/// its properties first; web URLs are left out.
pub fn read_note(md: &str) -> NoteText {
    let arena = Arena::new();
    let root = comrak::parse_document(&arena, md, &MdRender::comrak_options());
    let lines = line_starts(md);
    let properties = property_links(md, &lines, root)
        .into_iter()
        .map(|(found, key)| {
            let context = format!("{key}: {}", found.dest);
            let label = key.len() + 2..context.len();
            (found, (context, label))
        });
    let body = root
        .descendants()
        .filter_map(|node| Some((link_at(md, &lines, node)?, node)))
        .map(|(found, node)| (found, context(node)));
    let mut links = vec![];
    for (Found { kind, dest, span, written, .. }, (context, label)) in properties.chain(body) {
        if web_link(&dest) && link_id(&dest).is_none() {
            continue;
        }
        let rewritable = written.is_some();
        links.push(NoteLink { kind, dest, span, context, label, rewritable });
    }
    let headings = Arc::new(headings_in(root).into_iter().map(|(_, h)| h).collect());
    let properties = root
        .first_child()
        .and_then(|first| match &first.data.borrow().value {
            NodeValue::FrontMatter(text) => Some(properties_in(text)),
            _ => None,
        })
        .unwrap_or_default();
    NoteText { headings, links, properties }
}

/// The properties of a front matter block, as the grid reads them.
pub fn properties_in(block: &str) -> Vec<(String, Vec<String>)> {
    use crate::front_matter::{Quotes, RowKind, Value, parse, unquote};
    let Some(fm) = parse(block) else { return vec![] };
    let meaning = |text: &(usize, usize), quotes: &Quotes| match quotes {
        Some((open, close)) => unquote(&block[open.0..close.1]),
        None => Some(block[text.0..text.1].to_string()),
    };
    fm.rows
        .iter()
        .filter_map(|row| {
            let RowKind::Property { key, value, .. } = &row.kind else { return None };
            let values = match value {
                Value::Text { text, quotes, .. } if text.1 - text.0 <= PROPERTY_VALUE_BYTES => {
                    meaning(text, quotes).into_iter().collect()
                }
                Value::List { items, .. } => items
                    .iter()
                    .filter(|i| i.text.0 < i.text.1)
                    .filter_map(|i| meaning(&i.text, &i.quotes))
                    .collect(),
                _ => vec![],
            };
            Some((block[key.0..key.1].to_string(), values))
        })
        .collect()
}

/// A link as written: its kind, destination, bytes, and where its
/// destination is written in them.
struct Found {
    kind: LinkKind,
    dest: String,
    span: Range<usize>,
    written: Option<Range<usize>>,
    /// For a property's value, how it is written in YAML.
    yaml: Option<Yaml>,
}

/// How a property's value is written: its quote, if any, and whether it
/// is an item in brackets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Yaml {
    pub quote: Option<char>,
    pub flow: bool,
}

/// The links in a note's front matter, each with the key it is under:
/// every value and list item that is one URL. A value is rewritable when
/// it is written as it reads, with no escapes.
fn property_links<'a>(md: &str, lines: &[usize], root: &'a AstNode<'a>) -> Vec<(Found, String)> {
    let Some(first) = root.first_child() else { return vec![] };
    if !matches!(first.data.borrow().value, NodeValue::FrontMatter(_)) {
        return vec![];
    }
    let pos = first.data.borrow().sourcepos;
    let base = offset(md, lines, pos.start);
    let end = (offset(md, lines, pos.end) + 1).min(md.len());
    let Some(block) = md.get(base..end) else { return vec![] };
    let Some(fm) = front_matter::parse(block) else { return vec![] };
    fm.scalars()
        .filter_map(|s| {
            let (start, end) = s.written();
            let written = &block[start..end];
            let dest = url_value(written)?;
            let quote = s.quotes.and_then(|_| written.chars().next());
            let verbatim = block[s.text.0..s.text.1] == dest;
            let found = Found {
                kind: LinkKind::Link,
                span: base + start..base + end,
                written: verbatim.then_some(base + s.text.0..base + s.text.1),
                yaml: Some(Yaml { quote, flow: s.flow }),
                dest,
            };
            Some((found, block[s.key.0..s.key.1].to_string()))
        })
        .collect()
}

fn link_at<'a>(md: &str, lines: &[usize], node: &'a AstNode<'a>) -> Option<Found> {
    let (kind, dest) = match &node.data.borrow().value {
        NodeValue::Link(l) => (LinkKind::Link, l.url.clone()),
        NodeValue::Image(l) => (LinkKind::Image, l.url.clone()),
        NodeValue::WikiLink(l) => (LinkKind::Wiki, l.url.clone()),
        _ => return None,
    };
    let pos = node.data.borrow().sourcepos;
    let span = offset(md, lines, pos.start)..(offset(md, lines, pos.end) + 1).min(md.len());
    let written = md
        .get(span.clone())
        .and_then(|raw| dest_in(kind, raw, &dest))
        .map(|at| span.start + at.start..span.start + at.end);
    Some(Found { kind, dest, span, written, yaml: None })
}

/// A destination written where it can be rewritten.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Written {
    pub kind: LinkKind,
    /// The destination as read.
    pub dest: String,
    /// Its bytes.
    pub at: Range<usize>,
    /// The whole link's bytes, a property's quotes included.
    span: Range<usize>,
    yaml: Option<Yaml>,
}

impl Written {
    /// The bytes to replace, and what to write there, for the destination
    /// to read `to`. A property's value takes the quotes or escapes `to`
    /// needs there.
    pub fn rewrite(&self, to: &str) -> (Range<usize>, String) {
        let Some(Yaml { quote, flow }) = self.yaml else { return (self.at.clone(), to.into()) };
        let escaped = yaml_scalar(to, quote, flow);
        match quote {
            Some(_) if escaped == to => (self.at.clone(), escaped),
            Some(q) => (self.span.clone(), format!("{q}{escaped}{q}")),
            None => (self.at.clone(), escaped),
        }
    }
}

/// Where each link's destination is written in `md`, for rewriting it. A
/// destination that isn't written inside the link as it reads (a reference
/// link's, an escaped spelling) has none.
pub fn written_dests(md: &str) -> Vec<Written> {
    let arena = Arena::new();
    let root = comrak::parse_document(&arena, md, &MdRender::comrak_options());
    let lines = line_starts(md);
    let properties = property_links(md, &lines, root).into_iter().map(|(f, _)| f);
    let body = root
        .descendants()
        .filter_map(|node| link_at(md, &lines, node));
    properties
        .chain(body)
        .filter_map(|f| {
            let at = f.written?;
            Some(Written { kind: f.kind, dest: f.dest, at, span: f.span, yaml: f.yaml })
        })
        .collect()
}

/// The destination's bytes in a link's own source, where it's written as it
/// reads. A wiki title follows `[[` (`![[` for an embed); a destination
/// follows the first `](` it starts right after (a title may hold another);
/// an autolink is its own text. A reference link has none.
fn dest_in(kind: LinkKind, raw: &str, dest: &str) -> Option<Range<usize>> {
    let at = match kind {
        LinkKind::Wiki => {
            let open = if raw.starts_with('!') { 3 } else { 2 };
            raw.get(open - 2..)
                .and_then(|r| r.strip_prefix("[["))
                .filter(|title| title.starts_with(dest))
                .map(|_| open)
        }
        _ if raw.starts_with('[') || raw.starts_with("![") => raw
            .match_indices("](")
            .map(|(i, _)| {
                let after = &raw[i + 2..];
                let pad = after.len() - after.trim_start().len();
                let pointy = after[pad..].starts_with('<') as usize;
                i + 2 + pad + pointy
            })
            .find(|&at| raw[at..].starts_with(dest)),
        _ => raw.find(dest),
    }?;
    Some(at..at + dest.len())
}

/// Byte offset of each line's start.
fn line_starts(md: &str) -> Vec<usize> {
    let mut starts = vec![0];
    starts.extend(md.match_indices('\n').map(|(i, _)| i + 1));
    starts
}

fn offset(md: &str, lines: &[usize], at: LineColumn) -> usize {
    let line = lines
        .get(at.line.saturating_sub(1))
        .copied()
        .unwrap_or(md.len());
    (line + at.column.saturating_sub(1)).min(md.len())
}

/// The plain text of the block around `link` and where the link's own words
/// fall in it, trimmed to [`CONTEXT_BYTES`] on either side.
fn context<'a>(link: &'a AstNode<'a>) -> (String, Range<usize>) {
    let block = link
        .ancestors()
        .find(|n| {
            matches!(
                n.data.borrow().value,
                NodeValue::Paragraph | NodeValue::Heading(_) | NodeValue::TableCell
            )
        })
        .unwrap_or(link);
    let mut text = String::new();
    let mut label = 0..0;
    plain_text(block, link, &mut text, &mut label);
    if std::ptr::eq(block, link) {
        label = 0..text.len();
    }

    let start = floor_char(&text, label.start.saturating_sub(CONTEXT_BYTES));
    let end = ceil_char(&text, (label.end + CONTEXT_BYTES).min(text.len()));
    let mut out = String::new();
    // trims to whole words where it cuts
    let head = &text[start..label.start];
    let head = if start > 0 { head.split_once(' ').map_or(head, |(_, rest)| rest) } else { head };
    if start > 0 {
        out.push('…');
    }
    out.push_str(head.trim_start());
    let label_start = out.len();
    out.push_str(&text[label.clone()]);
    let label_end = out.len();
    let tail = &text[label.end..end];
    let tail =
        if end < text.len() { tail.rsplit_once(' ').map_or(tail, |(rest, _)| rest) } else { tail };
    out.push_str(tail.trim_end());
    if end < text.len() {
        out.push('…');
    }
    (out, label_start..label_end)
}

fn plain_text<'a>(
    node: &'a AstNode<'a>, link: &'a AstNode<'a>, out: &mut String, label: &mut Range<usize>,
) {
    for child in node.children() {
        let start = out.len();
        match &child.data.borrow().value {
            NodeValue::Text(t) => out.push_str(t),
            NodeValue::Code(c) => out.push_str(&c.literal),
            NodeValue::Math(m) => out.push_str(&m.literal),
            NodeValue::ShortCode(s) => out.push_str(&s.emoji),
            NodeValue::SoftBreak | NodeValue::LineBreak => out.push(' '),
            NodeValue::HtmlInline(_) | NodeValue::Raw(_) => {}
            _ => plain_text(child, link, out, label),
        }
        if std::ptr::eq(child, link) {
            *label = start..out.len();
        }
    }
}

fn floor_char(s: &str, mut i: usize) -> usize {
    while !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn ceil_char(s: &str, mut i: usize) -> usize {
    while !s.is_char_boundary(i) {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dests(md: &str) -> Vec<(LinkKind, String)> {
        read_note(md)
            .links
            .into_iter()
            .map(|l| (l.kind, l.dest))
            .collect()
    }

    #[test]
    fn reads_every_form_but_the_web() {
        let md = "See [plan](design/plan.md#goals), [[Budget|money]], ![chart](chart.png)\n\
                  and https://example.com and <lb://00000000-0000-0000-0000-000000000001>.";
        assert_eq!(
            dests(md),
            [
                (LinkKind::Link, "design/plan.md#goals".into()),
                (LinkKind::Wiki, "Budget".into()),
                (LinkKind::Image, "chart.png".into()),
                (LinkKind::Link, "lb://00000000-0000-0000-0000-000000000001".into()),
            ]
        );
    }

    /// What a mend rewrites is the destination as written, nothing around it.
    #[test]
    fn finds_each_destination_where_it_is_written() {
        let md = "---\ntitle: x\n---\n# [[Plan#Goals|the plan]]\n\n| a | [b](../b%20c.md \"t\") |\n|---|---|\n\n- ![](img.png) ![[Chart.png|300]]\n";
        let found: Vec<&str> = written_dests(md).into_iter().map(|w| &md[w.at]).collect();
        assert_eq!(found, ["Plan#Goals", "../b%20c.md", "img.png", "Chart.png"]);
    }

    /// Only a destination written in the link, as it reads, is rewritten:
    /// not a reference link's label, a title, or an escaped spelling.
    #[test]
    fn rewritable_destinations() {
        let md = "[plan.md] [x][plan.md] [y](p.md 'a](p.md') [z](a\\_b.md) [[ Plan ]]\n\n[plan.md]: plan.md\n";
        let links = read_note(md).links;
        let rewritable: Vec<(&str, bool)> = links
            .iter()
            .map(|l| (l.dest.as_str(), l.rewritable))
            .collect();
        assert_eq!(
            rewritable,
            [
                ("plan.md", false),
                ("plan.md", false),
                ("p.md", true),
                ("a_b.md", false),
                ("Plan", false)
            ]
        );
        let at = written_dests(md)
            .into_iter()
            .find(|w| w.dest == "p.md")
            .unwrap()
            .at;
        assert_eq!(at.start, md.find("(p.md").unwrap() + 1);
    }

    /// A property value, or a list item, that is one URL is a link of its
    /// own, read from where it is written.
    #[test]
    fn reads_links_in_properties() {
        let a = "lb://00000000-0000-0000-0000-000000000001";
        let b = "lb://00000000-0000-0000-0000-000000000002";
        let md = format!(
            "---\nsession: {a}\nblockers: [{b}, '{a}']\nsee:\n  - {b}\n\
             site: https://example.com\nnot: lb:// x\n---\n[[body]]\n"
        );
        let links = read_note(&md).links;
        let found: Vec<(&str, &str, &str)> = links
            .iter()
            .map(|l| (l.dest.as_str(), &md[l.span.clone()], &l.context[l.label.clone()]))
            .collect();
        let quoted = format!("'{a}'");
        assert_eq!(
            found,
            [(a, a, a), (b, b, b), (a, &quoted, a), (b, b, b), ("body", "[[body]]", "body")]
        );
        assert_eq!(links[1].context, format!("blockers: {b}"));
        assert!(links.iter().all(|l| l.rewritable));
    }

    #[test]
    fn context_is_the_block_with_the_link_marked() {
        let md = "Meeting notes: see **[the plan](plan.md)** before `Friday`.";
        let link = &read_note(md).links[0];
        assert_eq!(link.context, "Meeting notes: see the plan before Friday.");
        assert_eq!(&link.context[link.label.clone()], "the plan");
    }

    #[test]
    fn long_context_is_cut_at_words() {
        let words = "word ".repeat(100);
        let md = format!("{words}[[Target]] {words}");
        let link = &read_note(&md).links[0];
        assert!(link.context.starts_with('…') && link.context.ends_with('…'));
        assert!(link.context.len() < 2 * CONTEXT_BYTES + 20);
        assert_eq!(&link.context[link.label.clone()], "Target");
        assert!(!link.context.contains("wor…") && !link.context.contains("…ord"));
    }
}
