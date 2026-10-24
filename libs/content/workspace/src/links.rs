//! The links in a note's text, read as the editor reads them: what each one
//! is written with, and the words around it.

use std::ops::Range;
use std::sync::Arc;

use comrak::Arena;
use comrak::nodes::{AstNode, LineColumn, NodeValue};

use crate::file_cache::{link_id, web_link};
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
}

/// Plain text kept on either side of a link's own words.
const CONTEXT_BYTES: usize = 160;

/// Reads a note's headings and the links that may reach a file; web URLs
/// are left out.
pub fn read_note(md: &str) -> NoteText {
    let arena = Arena::new();
    let root = comrak::parse_document(&arena, md, &MdRender::comrak_options());
    let lines = line_starts(md);
    let mut links = vec![];
    for node in root.descendants() {
        let Some(Found { kind, dest, span, written }) = link_at(md, &lines, node) else {
            continue;
        };
        if web_link(&dest) && link_id(&dest).is_none() {
            continue;
        }
        let (context, label) = context(node);
        let rewritable = written.is_some();
        links.push(NoteLink { kind, dest, span, context, label, rewritable });
    }
    let headings = Arc::new(headings_in(root).into_iter().map(|(_, h)| h).collect());
    NoteText { headings, links }
}

/// A link node as written: its kind, destination, bytes, and where its
/// destination is written in them.
struct Found {
    kind: LinkKind,
    dest: String,
    span: Range<usize>,
    written: Option<Range<usize>>,
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
    Some(Found { kind, dest, span, written })
}

/// Where each link's destination is written in `md`, for rewriting it:
/// the kind, the destination as read, and its bytes. A destination that
/// isn't written inside the link (a reference link's) has none.
pub fn written_dests(md: &str) -> Vec<(LinkKind, String, Range<usize>)> {
    let arena = Arena::new();
    let root = comrak::parse_document(&arena, md, &MdRender::comrak_options());
    let lines = line_starts(md);
    root.descendants()
        .filter_map(|node| link_at(md, &lines, node))
        .filter_map(|f| Some((f.kind, f.dest, f.written?)))
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
        let found: Vec<&str> = written_dests(md)
            .into_iter()
            .map(|(_, _, at)| &md[at])
            .collect();
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
            .find(|(_, d, _)| d == "p.md")
            .unwrap()
            .2;
        assert_eq!(at.start, md.find("(p.md").unwrap() + 1);
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
