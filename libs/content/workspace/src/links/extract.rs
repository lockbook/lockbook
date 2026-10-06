use std::ops::Range;

use comrak::Arena;
use comrak::nodes::{LineColumn, NodeValue};

use crate::tab::markdown_editor::MdRender;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum LinkKind {
    /// `[label](destination)`, or a bare URL.
    Link,
    /// `![alt](destination)`.
    Image,
    /// `[[title]]` or `[[title|label]]`.
    Wiki,
}

/// A link as it is written in a note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    pub kind: LinkKind,
    /// What the link points at, as the editor reads it: a URL or path, or a
    /// wikilink's title.
    pub dest: String,
    /// Where the link's whole syntax and the destination inside it sit in the
    /// source, in bytes. `None` for a destination written somewhere else (a
    /// reference link's definition).
    pub spans: Option<Spans>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spans {
    pub link: Range<usize>,
    pub dest: Range<usize>,
}

/// The links in a note's source, in the order they are written. Parses as the
/// editor does, so a link here is a link there.
pub fn extract(text: &str) -> Vec<Link> {
    let arena = Arena::new();
    let source = format!("{text}\n");
    let root = comrak::parse_document(&arena, &source, &MdRender::comrak_options());

    let lines = line_starts(text);
    let offset = |at: LineColumn| {
        let line = lines.get(at.line.saturating_sub(1)).copied();
        (line.unwrap_or(text.len()) + at.column.saturating_sub(1)).min(text.len())
    };

    let mut links = vec![];
    for node in root.descendants() {
        let data = node.data.borrow();
        let (kind, dest) = match &data.value {
            NodeValue::Link(link) => (LinkKind::Link, &link.url),
            NodeValue::Image(link) => (LinkKind::Image, &link.url),
            NodeValue::WikiLink(link) => (LinkKind::Wiki, &link.url),
            _ => continue,
        };
        let span = offset(data.sourcepos.start)..(offset(data.sourcepos.end) + 1).min(text.len());
        let spans = text
            .get(span.clone())
            .and_then(|raw| dest_span(kind, raw, dest))
            .map(|dest| Spans { dest: span.start + dest.start..span.start + dest.end, link: span });
        links.push(Link { kind, dest: dest.clone(), spans });
    }
    links
}

/// Byte offset of each line's start. Line endings are CommonMark's: `\n`,
/// `\r\n`, or a lone `\r`.
fn line_starts(text: &str) -> Vec<usize> {
    let bytes = text.as_bytes();
    let mut starts = vec![0];
    let mut i = 0;
    while i < bytes.len() {
        i += match bytes[i] {
            b'\r' if bytes.get(i + 1) == Some(&b'\n') => 2,
            b'\r' | b'\n' => 1,
            _ => {
                i += 1;
                continue;
            }
        };
        starts.push(i);
    }
    starts
}

/// Where the destination sits in a link's own source, `raw`. `None` when the
/// source doesn't have the shape its kind is written in, or `dest` isn't
/// what is written there.
fn dest_span(kind: LinkKind, raw: &str, dest: &str) -> Option<Range<usize>> {
    let trimmed = |range: Range<usize>| {
        let inner = &raw[range.clone()];
        let start = range.start + (inner.len() - inner.trim_start().len());
        start..start + inner.trim().len()
    };
    let lead = if kind == LinkKind::Image { "![" } else { "[" };
    let span = match kind {
        LinkKind::Wiki => {
            let inner = raw.strip_prefix("[[")?.strip_suffix("]]")?;
            let title = unescaped_find(inner, '|').unwrap_or(inner.len());
            trimmed(2..2 + title)
        }
        _ if raw.starts_with(lead) && raw.contains("](") => {
            // `(destination "title")`
            let inside = raw.rfind("](")? + 2..raw.strip_suffix(')')?.len();
            let start = trimmed(inside.clone()).start;
            let rest = &raw[start..inside.end];
            // a bare destination runs to a space or a control character
            let ends = |c: char| c == ' ' || c.is_ascii_control();
            match rest.strip_prefix('<') {
                Some(pointy) => start + 1..start + 1 + unescaped_find(pointy, '>')?,
                None => start..start + rest.find(ends).unwrap_or(rest.len()),
            }
        }
        _ => {
            // a bare or `<pointy>` URL is its own destination; the parser may
            // have put a scheme in front of it
            let pointy = raw.strip_prefix('<').and_then(|r| r.strip_suffix('>'));
            let span = if pointy.is_some() { 1..raw.len() - 1 } else { 0..raw.len() };
            let auto =
                kind == LinkKind::Link && !raw.is_empty() && dest.ends_with(&raw[span.clone()]);
            return auto.then_some(span);
        }
    };
    // the parser unescapes what it reads; anything else it reads as written
    let written = &raw[span.clone()];
    (written.contains(['\\', '&']) || written == dest).then_some(span)
}

/// Byte index of the first `c` in `s` that no backslash escapes.
fn unescaped_find(s: &str, c: char) -> Option<usize> {
    let mut escaped = false;
    for (i, ch) in s.char_indices() {
        if !escaped && ch == c {
            return Some(i);
        }
        escaped = !escaped && ch == '\\';
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dests(text: &str) -> Vec<(LinkKind, String, Option<String>)> {
        extract(text)
            .into_iter()
            .map(|l| (l.kind, l.dest, l.spans.map(|s| text[s.dest].to_string())))
            .collect()
    }

    #[test]
    fn finds_each_form() {
        use LinkKind::*;
        let text = "\
see [the plan](../plan.md), [[Budget]], and [[design/Plan|that one]].\r\n\
![chart](imports/chart%20one.png \"Q3\") then <https://lockbook.net> or https://example.com/a.\n\
- [spaced]( <my notes/a b.md> ) and [empty]()\n";
        assert_eq!(
            dests(text),
            [
                (Link, "../plan.md".into(), Some("../plan.md".into())),
                (Wiki, "Budget".into(), Some("Budget".into())),
                (Wiki, "design/Plan".into(), Some("design/Plan".into())),
                (Image, "imports/chart%20one.png".into(), Some("imports/chart%20one.png".into())),
                (Link, "https://lockbook.net".into(), Some("https://lockbook.net".into())),
                (Link, "https://example.com/a".into(), Some("https://example.com/a".into())),
                (Link, "my notes/a b.md".into(), Some("my notes/a b.md".into())),
                (Link, "".into(), Some("".into())),
            ]
        );
    }

    #[test]
    fn a_destination_ends_where_the_parser_ends_it() {
        let text = "![x](R&amp;D/a\u{3000}b.png) [a](<b\\>c.md>) [n](a\u{a0}b.md 't')";
        let read: Vec<_> = dests(text)
            .into_iter()
            .map(|(_, dest, at)| (dest, at))
            .collect();
        assert_eq!(
            read,
            [
                ("R&D/a\u{3000}b.png".into(), Some("R&amp;D/a\u{3000}b.png".into())),
                ("b>c.md".into(), Some("b\\>c.md".into())),
                ("a\u{a0}b.md".into(), Some("a\u{a0}b.md".into())),
            ]
        );
    }

    #[test]
    fn code_is_not_a_link_and_references_have_no_span() {
        let text = "`[a](b.md)` and\n\n```\n[[Plan]]\n```\n\n[ref][r]\n\n[r]: /x.md\n";
        assert_eq!(dests(text), [(LinkKind::Link, "/x.md".into(), None)]);
    }

    #[test]
    fn an_image_in_a_link_is_both() {
        let text = "[![alt](shot.png)](page.md)";
        let links = extract(text);
        assert_eq!(links.len(), 2);
        let spans: Vec<_> = links.iter().map(|l| l.spans.clone().unwrap()).collect();
        assert_eq!(&text[spans[0].link.clone()], text);
        assert_eq!(&text[spans[0].dest.clone()], "page.md");
        assert_eq!(&text[spans[1].link.clone()], "![alt](shot.png)");
        assert_eq!(&text[spans[1].dest.clone()], "shot.png");
    }

    /// Whatever is written into a destination's span is what the link then
    /// points at.
    #[test]
    fn a_rewritten_destination_reads_back() {
        let text = "# Trip\n\n> [a](old.md) ![b](<old one.png> 'x') [[Old|label]] \\[not](one)\n\n| [c](old.md) |\n|-|\n";
        let links = extract(text);
        assert_eq!(links.len(), 4);
        for (i, link) in links.iter().enumerate() {
            let spans = link.spans.clone().unwrap();
            let new = if link.kind == LinkKind::Wiki { "folder/New" } else { "folder/new.md" };
            let mut rewritten = text.to_string();
            rewritten.replace_range(spans.dest, new);
            assert_eq!(extract(&rewritten)[i].dest, new, "{rewritten}");
        }
    }
}
