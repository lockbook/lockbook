//! Heading fragments in markdown and wikilinks (`note.md#heading`, `[[note#heading]]`,
//! `[[#heading]]`).
//!
//! Matching is generous so both humans and models land on the right heading:
//! GitHub-style unique slugs (`#link-fragments`), percent-decoded text
//! (`#Link%20Fragments`), and case-insensitive heading text (`#Link Fragments`).
//! Duplicate headings get GitHub's `-1`, `-2`, … suffix in document order.

use std::collections::HashMap;

use comrak::nodes::{AstNode, NodeValue};
use lb_rs::model::text::offset_types::Grapheme;
use urlencoding::decode;

use super::MdRender;

pub use crate::file_cache::split_internal_fragment;

/// GitHub-slugger-ish: lowercase, keep letters/digits, collapse everything
/// else to a single hyphen, trim hyphens.
pub fn slugify(text: &str) -> String {
    let mut out = String::new();
    let mut hyphen = false;
    for c in text.chars() {
        if c.is_alphanumeric() {
            for lower in c.to_lowercase() {
                out.push(lower);
            }
            hyphen = false;
        } else if !hyphen && !out.is_empty() {
            out.push('-');
            hyphen = true;
        }
    }
    if out.ends_with('-') {
        out.pop();
    }
    out
}

fn heading_plain<'a>(node: &'a AstNode<'a>) -> String {
    let mut out = String::new();
    for n in node.descendants() {
        match &n.data.borrow().value {
            NodeValue::Text(t) => out.push_str(t),
            NodeValue::Code(c) => out.push_str(&c.literal),
            _ => {}
        }
    }
    out
}

fn decode_fragment(fragment: &str) -> String {
    decode(fragment)
        .map(|c| c.into_owned())
        .unwrap_or_else(|_| fragment.to_string())
}

/// True when `fragment` identifies this heading. `slug` is the heading's
/// unique GitHub-style id (already de-duplicated).
pub fn fragment_matches(heading_text: &str, slug: &str, fragment: &str) -> bool {
    let frag = decode_fragment(fragment);
    let frag = frag.trim();
    if frag.is_empty() || slug.is_empty() {
        return false;
    }
    if frag == slug || slugify(frag) == slug {
        return true;
    }
    heading_text.trim().eq_ignore_ascii_case(frag)
}

/// Walk headings in document order, assigning unique slugs, and return the
/// source range of the first match. Unique-slug hits win immediately so
/// `#foo-1` is the second "Foo", not a later heading whose text matches.
pub fn find_heading_range<'ast>(
    md: &MdRender, root: &'ast AstNode<'ast>, fragment: &str,
) -> Option<(Grapheme, Grapheme)> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut text_fallback: Option<(Grapheme, Grapheme)> = None;
    for node in root.descendants() {
        if !matches!(node.data.borrow().value, NodeValue::Heading(_)) {
            continue;
        }
        let text = heading_plain(node);
        let base = slugify(&text);
        if base.is_empty() {
            continue;
        }
        let n = seen.entry(base.clone()).or_insert(0);
        let slug = if *n == 0 { base.clone() } else { format!("{base}-{n}") };
        *n += 1;
        if frag_is_slug_match(&slug, fragment) {
            return Some(md.node_range(node));
        }
        if text_fallback.is_none() && heading_text_matches(&text, fragment) {
            text_fallback = Some(md.node_range(node));
        }
    }
    text_fallback
}

fn frag_is_slug_match(slug: &str, fragment: &str) -> bool {
    let frag = decode_fragment(fragment);
    let frag = frag.trim();
    frag == slug || slugify(frag) == slug
}

fn heading_text_matches(heading_text: &str, fragment: &str) -> bool {
    let frag = decode_fragment(fragment);
    heading_text.trim().eq_ignore_ascii_case(frag.trim())
}

/// How a dest was written in the source. Resolve at query time via FileCache.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkKind {
    Markdown,
    Wiki,
    Embed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outgoing {
    pub dest: String,
    pub kind: LinkKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DocumentExtract {
    pub headings: Vec<(String, String)>,
    pub outgoing: Vec<Outgoing>,
}

/// One heading in document order, for the outline sidecar.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutlineItem {
    pub level: u8,
    pub text: String,
    pub slug: String,
}

/// How to write a copied heading dest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeadingLinkKind {
    Wiki,
    Markdown,
    Share,
}

/// Unique GitHub-style slug for this heading node (document order).
pub fn slug_for_heading_node<'a>(node: &'a AstNode<'a>) -> Option<(String, String)> {
    let mut root = node;
    while let Some(p) = root.parent() {
        root = p;
    }
    let mut seen: HashMap<String, usize> = HashMap::new();
    for n in root.descendants() {
        let NodeValue::Heading(_) = &n.data.borrow().value else { continue };
        let text = heading_plain(n);
        let base = slugify(&text);
        if base.is_empty() {
            continue;
        }
        let k = seen.entry(base.clone()).or_insert(0);
        let slug = if *k == 0 { base.clone() } else { format!("{base}-{k}") };
        *k += 1;
        if std::ptr::eq(n, node) {
            return Some((text, slug));
        }
    }
    None
}

/// Headings of `root` as outline rows (unique GitHub-style slugs).
pub fn outline_from_ast<'a>(root: &'a AstNode<'a>) -> Vec<OutlineItem> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut out = Vec::new();
    for node in root.descendants() {
        let NodeValue::Heading(h) = &node.data.borrow().value else {
            continue;
        };
        let text = heading_plain(node);
        let base = slugify(&text);
        if base.is_empty() {
            continue;
        }
        let n = seen.entry(base.clone()).or_insert(0);
        let slug = if *n == 0 { base.clone() } else { format!("{base}-{n}") };
        *n += 1;
        out.push(OutlineItem { level: h.level, text, slug });
    }
    out
}

/// One comrak pass: headings and outgoing dests (md / wiki / image).
/// External `http(s)` / `mailto` dests are omitted — they aren't backlinks.
pub fn extract_document(md: &str) -> DocumentExtract {
    let arena = comrak::Arena::new();
    let root = comrak::parse_document(&arena, md, &super::MdRender::comrak_options());
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut headings = Vec::new();
    let mut outgoing = Vec::new();
    for node in root.descendants() {
        match &node.data.borrow().value {
            NodeValue::Heading(_) => {
                let text = heading_plain(node);
                let base = slugify(&text);
                if base.is_empty() {
                    continue;
                }
                let n = seen.entry(base.clone()).or_insert(0);
                let slug = if *n == 0 { base.clone() } else { format!("{base}-{n}") };
                *n += 1;
                headings.push((text, slug));
            }
            NodeValue::WikiLink(nwl) => push_outgoing(&mut outgoing, &nwl.url, LinkKind::Wiki),
            NodeValue::Link(nl) => push_outgoing(&mut outgoing, &nl.url, LinkKind::Markdown),
            NodeValue::Image(ni) => push_outgoing(&mut outgoing, &ni.url, LinkKind::Embed),
            _ => {}
        }
    }
    DocumentExtract { headings, outgoing }
}

fn push_outgoing(out: &mut Vec<Outgoing>, dest: &str, kind: LinkKind) {
    if dest.is_empty() {
        return;
    }
    if dest.starts_with("http://") || dest.starts_with("https://") || dest.starts_with("mailto:") {
        return;
    }
    out.push(Outgoing { dest: dest.to_string(), kind });
}

/// Replace dests in wiki / markdown / embed syntax. Closure returns the new
/// dest (including fragment) or `None` to leave this occurrence.
///
/// Only the dest inside `[[…]]` or `](…)` is touched — prose is left alone.
pub fn rewrite_outgoing_dests(
    md: &str, mut next: impl FnMut(&Outgoing) -> Option<String>,
) -> String {
    let arena = comrak::Arena::new();
    let root = comrak::parse_document(&arena, md, &super::MdRender::comrak_options());
    let mut reps: Vec<(usize, usize, String)> = Vec::new();
    for node in root.descendants() {
        let (dest, kind) = {
            let data = node.data.borrow();
            match &data.value {
                NodeValue::WikiLink(nwl) => (nwl.url.clone(), LinkKind::Wiki),
                NodeValue::Link(nl) => (nl.url.clone(), LinkKind::Markdown),
                NodeValue::Image(ni) => (ni.url.clone(), LinkKind::Embed),
                _ => continue,
            }
        };
        if dest.is_empty()
            || dest.starts_with("http://")
            || dest.starts_with("https://")
            || dest.starts_with("mailto:")
        {
            continue;
        }
        let o = Outgoing { dest, kind };
        let Some(new) = next(&o) else { continue };
        if new == o.dest {
            continue;
        }
        let sp = node.data.borrow().sourcepos;
        let (ns, ne) =
            sourcepos_bytes(md, sp.start.line, sp.start.column, sp.end.line, sp.end.column);
        if let Some((s, e)) = dest_span(md, ns, ne, &o) {
            reps.push((s, e, new));
        }
    }
    reps.sort_by_key(|(s, _, _)| *s);
    let mut out = String::with_capacity(md.len());
    let mut i = 0usize;
    for (s, e, new) in reps {
        if s < i || e > md.len() || s > e {
            continue;
        }
        out.push_str(&md[i..s]);
        out.push_str(&new);
        i = e;
    }
    out.push_str(&md[i..]);
    out
}

fn sourcepos_bytes(md: &str, sl: usize, sc: usize, el: usize, ec: usize) -> (usize, usize) {
    let start = line_col_to_byte(md, sl, sc);
    let end = line_col_to_byte(md, el, ec.saturating_add(1));
    (start.min(md.len()), end.min(md.len()))
}

fn line_col_to_byte(md: &str, line: usize, column: usize) -> usize {
    let mut left = line.saturating_sub(1);
    let mut start = 0usize;
    if left > 0 {
        for (i, b) in md.bytes().enumerate() {
            if b == b'\n' {
                left -= 1;
                start = i + 1;
                if left == 0 {
                    break;
                }
            }
        }
    }
    start.saturating_add(column.saturating_sub(1))
}

fn dest_span(md: &str, node_start: usize, node_end: usize, o: &Outgoing) -> Option<(usize, usize)> {
    let end = node_end.min(md.len());
    let start = node_start.min(end);
    let slice = md.get(start..end)?;
    let needle = o.dest.as_str();
    let rel = match o.kind {
        LinkKind::Wiki => {
            let open = slice.find("[[")?;
            let after = open + 2;
            slice.get(after..)?.starts_with(needle).then_some(after)
        }
        LinkKind::Markdown | LinkKind::Embed => {
            let open = slice.rfind("](")?;
            let after = open + 2;
            let rest = slice.get(after..)?;
            if rest.starts_with(needle) {
                Some(after)
            } else if rest.starts_with('<') && rest[1..].starts_with(needle) {
                Some(after + 1)
            } else {
                None
            }
        }
    }?;
    let abs = start + rel;
    Some((abs, abs + needle.len()))
}

/// Headings of `md` as `(display text, unique slug)` in document order.
pub fn document_headings(md: &str) -> Vec<(String, String)> {
    extract_document(md).headings
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_keeps_external_fragments() {
        assert_eq!(
            split_internal_fragment("https://example.com/a#b"),
            ("https://example.com/a#b", None)
        );
        assert_eq!(split_internal_fragment("mailto:x@y.com#inbox"), ("mailto:x@y.com#inbox", None));
    }

    #[test]
    fn split_internal() {
        assert_eq!(split_internal_fragment("note.md#Hello"), ("note.md", Some("Hello")));
        assert_eq!(split_internal_fragment("#Hello"), ("", Some("Hello")));
        assert_eq!(split_internal_fragment("lb://abc#Hello"), ("lb://abc", Some("Hello")));
        assert_eq!(split_internal_fragment("[[note]]"), ("[[note]]", None));
        assert_eq!(split_internal_fragment("note.md"), ("note.md", None));
    }

    #[test]
    fn slugify_githubish() {
        assert_eq!(slugify("Link Fragments"), "link-fragments");
        assert_eq!(slugify("  Hello, World!  "), "hello-world");
        assert_eq!(slugify("foo_bar"), "foo-bar");
        assert_eq!(slugify("C++"), "c");
        assert_eq!(slugify("déjà vu"), "déjà-vu");
    }

    #[test]
    fn match_slug_text_and_percent() {
        assert!(fragment_matches("Link Fragments", "link-fragments", "link-fragments"));
        assert!(fragment_matches("Link Fragments", "link-fragments", "Link Fragments"));
        assert!(fragment_matches("Link Fragments", "link-fragments", "Link%20Fragments"));
        assert!(!fragment_matches("Link Fragments", "link-fragments", "other"));
    }

    #[test]
    fn document_headings_unique_slugs() {
        let md = "# Foo\n\n# Bar\n\n# Foo\n";
        let hs = document_headings(md);
        assert_eq!(
            hs,
            vec![
                ("Foo".into(), "foo".into()),
                ("Bar".into(), "bar".into()),
                ("Foo".into(), "foo-1".into()),
            ]
        );
    }

    #[test]
    fn outline_keeps_level() {
        let arena = comrak::Arena::new();
        let root = comrak::parse_document(
            &arena,
            "# Title\n\n## Nest\n\n### Deep\n",
            &super::MdRender::comrak_options(),
        );
        let items = outline_from_ast(root);
        assert_eq!(
            items,
            vec![
                OutlineItem { level: 1, text: "Title".into(), slug: "title".into() },
                OutlineItem { level: 2, text: "Nest".into(), slug: "nest".into() },
                OutlineItem { level: 3, text: "Deep".into(), slug: "deep".into() },
            ]
        );
    }

    #[test]
    fn extract_outgoing_skips_external() {
        let ex = extract_document(
            "see [[note#Hello]] and [x](other.md#Frag) and ![](pic.png)\n\nhttps://ex.com/a#b\n",
        );
        assert_eq!(
            ex.outgoing,
            vec![
                Outgoing { dest: "note#Hello".into(), kind: LinkKind::Wiki },
                Outgoing { dest: "other.md#Frag".into(), kind: LinkKind::Markdown },
                Outgoing { dest: "pic.png".into(), kind: LinkKind::Embed },
            ]
        );
    }

    #[test]
    fn rewrite_touches_syntax_not_prose() {
        let md = "see [[note]] and note and [x](other.md#Frag) and ![](pic.png)\n";
        let out = rewrite_outgoing_dests(md, |o| match (o.kind, o.dest.as_str()) {
            (LinkKind::Wiki, "note") => Some("journal".into()),
            (LinkKind::Markdown, "other.md#Frag") => Some("b.md#Frag".into()),
            (LinkKind::Embed, "pic.png") => Some("img.png".into()),
            _ => None,
        });
        assert_eq!(out, "see [[journal]] and note and [x](b.md#Frag) and ![](img.png)\n");
    }

    #[test]
    fn rewrite_wiki_keeps_title() {
        let md = "[[note.md#Hello|Note]]\n";
        let out = rewrite_outgoing_dests(md, |_| Some("journal.md#Hello".into()));
        assert_eq!(out, "[[journal.md#Hello|Note]]\n");
    }
}
