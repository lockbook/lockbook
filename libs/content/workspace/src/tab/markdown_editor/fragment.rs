//! Heading fragments: the `#…` after a destination.
//!
//! Markdown destinations carry a GitHub slug (`note.md#link-fragments`),
//! wiki destinations the heading text (`[[Note#Link Fragments]]`).
//! Duplicates number the way GitHub numbers them: `foo`, `foo-1`, `foo-1-1`.
//! A fragment names a heading by its slug first, then by its text.

use std::collections::HashSet;

use comrak::nodes::{AstNode, NodeValue};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};

use super::MdRender;

/// One heading of a note, in document order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Heading {
    pub level: u8,
    pub text: String,
    pub slug: String,
}

/// Lowercase; whitespace becomes `-`; letters, numbers, `_`, and `-` stay.
pub fn slugify(text: &str) -> String {
    use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory as _};
    let mut out = String::new();
    for c in text.trim().chars() {
        if c.is_whitespace() {
            out.push('-');
        } else if c.is_alphanumeric()
            || c == '_'
            || c == '-'
            || c.general_category_group() == GeneralCategoryGroup::Mark
        {
            out.extend(c.to_lowercase());
        }
    }
    out
}

/// `text`'s slug, numbered past the ones already `taken`. None for a heading
/// of only punctuation, which no slug names.
fn unique_slug(taken: &mut HashSet<String>, text: &str) -> Option<String> {
    let base = slugify(text);
    if base.is_empty() {
        return None;
    }
    let slug = (0..)
        .map(|n| if n == 0 { base.clone() } else { format!("{base}-{n}") })
        .find(|slug| !taken.contains(slug))?;
    taken.insert(slug.clone());
    Some(slug)
}

/// The heading as plain text: its words, code, and emoji, without markup.
pub fn heading_text<'a>(node: &'a AstNode<'a>) -> String {
    let mut out = String::new();
    for n in node.descendants() {
        match &n.data.borrow().value {
            NodeValue::Text(t) => out.push_str(t),
            NodeValue::Code(c) => out.push_str(&c.literal),
            NodeValue::ShortCode(s) => out.push_str(&s.emoji),
            _ => {}
        }
    }
    out.trim().to_string()
}

/// Every heading under `root` that a fragment can name, in document order.
pub fn headings_in<'a>(root: &'a AstNode<'a>) -> Vec<(&'a AstNode<'a>, Heading)> {
    let mut taken = HashSet::new();
    let mut out = Vec::new();
    for node in root.descendants() {
        let NodeValue::Heading(h) = node.data.borrow().value else { continue };
        let text = heading_text(node);
        if let Some(slug) = unique_slug(&mut taken, &text) {
            out.push((node, Heading { level: h.level, text, slug }));
        }
    }
    out
}

/// The headings of markdown source.
pub fn document_headings(md: &str) -> Vec<Heading> {
    let arena = comrak::Arena::new();
    let root = comrak::parse_document(&arena, md, &MdRender::comrak_options());
    headings_in(root).into_iter().map(|(_, h)| h).collect()
}

/// The headings above `headings[i]`, outermost first: `A / B`.
pub fn crumbs(headings: &[Heading], i: usize) -> String {
    let mut chain = Vec::new();
    let mut level = headings[i].level;
    for h in headings[..i].iter().rev() {
        if h.level < level {
            chain.push(h.text.as_str());
            level = h.level;
        }
    }
    chain.reverse();
    chain.join(" / ")
}

/// Which of `headings` a fragment names: a slug first, so `#foo-1` is the
/// second "Foo" and never a heading titled "foo-1"; then the first heading
/// with that text. Case doesn't count; percent-escapes are decoded.
pub fn find_heading(headings: &[Heading], fragment: &str) -> Option<usize> {
    let fragment = urlencoding::decode(fragment).map_or(fragment.into(), |f| f.into_owned());
    let fragment = fragment.trim().to_lowercase();
    if fragment.is_empty() {
        return None;
    }
    headings
        .iter()
        .position(|h| h.slug == fragment)
        .or_else(|| {
            headings
                .iter()
                .position(|h| h.text.to_lowercase() == fragment)
        })
}

/// What a wiki link puts after `#`: the heading's text, unless that names an
/// earlier heading or would end the link.
pub fn wiki_token(heading: &Heading) -> String {
    let text = &heading.text;
    let plain = !text.contains(['#', '|', '[', ']', '\\']);
    if plain && slugify(text) == heading.slug { text.clone() } else { heading.slug.clone() }
}

impl<'ast> MdRender {
    /// Where a jump to a heading puts the caret: after its text, before
    /// trailing markup such as a fold tag.
    pub fn heading_caret(&self, node: &'ast AstNode<'ast>) -> Grapheme {
        let last = node
            .children()
            .filter(|c| !matches!(c.data.borrow().value, NodeValue::HtmlInline(_)))
            .last();
        match last {
            Some(child) => self.node_range(child).end(),
            None => self.node_range(node).end(),
        }
    }

    /// Where a heading's text starts and ends. Its markup may be hidden, with
    /// nothing laid out to scroll to, so the text stands for the heading.
    pub fn heading_span(&self, node: &'ast AstNode<'ast>) -> (Grapheme, Grapheme) {
        let start = node
            .first_child()
            .map_or(self.node_range(node).0, |c| self.node_range(c).0);
        (start, self.heading_caret(node))
    }

    /// The slug of the heading holding `offset`, when it has one.
    pub fn heading_slug_at(&self, root: &'ast AstNode<'ast>, offset: Grapheme) -> Option<String> {
        headings_in(root)
            .into_iter()
            .find(|(node, _)| self.node_range(node).contains(offset, true, true))
            .map(|(_, heading)| heading.slug)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slugs_number_duplicates_like_github() {
        let md =
            "# Links\n\n## Link `Fragments` :smile:\n\n### Foo\n\n## Foo\n\n# Foo 1\n\n# !!!\n";
        let flat: Vec<(u8, String, String)> = document_headings(md)
            .into_iter()
            .map(|h| (h.level, h.text, h.slug))
            .collect();
        let row = |level, text: &str, slug: &str| (level, text.to_string(), slug.to_string());
        assert_eq!(
            flat,
            vec![
                row(1, "Links", "links"),
                row(2, "Link Fragments 😄", "link-fragments-"),
                row(3, "Foo", "foo"),
                row(2, "Foo", "foo-1"),
                row(1, "Foo 1", "foo-1-1"),
            ]
        );
        assert_eq!(slugify("  Hello, World!  "), "hello-world");
        assert_eq!(slugify("déjà vu"), "déjà-vu");
        assert_eq!(slugify("नमस्ते दुनिया"), "नमस्ते-दुनिया");
    }

    #[test]
    fn a_slug_wins_over_text() {
        let headings = document_headings("# Foo\n\n# Foo\n\n# Foo 1\n\n# foo-1\n");
        assert_eq!(find_heading(&headings, "foo"), Some(0));
        assert_eq!(find_heading(&headings, "FOO"), Some(0));
        assert_eq!(find_heading(&headings, "foo-1"), Some(1));
        assert_eq!(find_heading(&headings, "Foo 1"), Some(2));
        assert_eq!(find_heading(&headings, "Foo%201"), Some(2));
        assert_eq!(find_heading(&headings, "foo-1-2"), Some(3));
        assert_eq!(find_heading(&headings, "bar"), None);
        assert_eq!(find_heading(&headings, ""), None);
        // every heading is named by its wiki token
        for (i, h) in headings.iter().enumerate() {
            assert_eq!(find_heading(&headings, &wiki_token(h)), Some(i), "{h:?}");
        }
        assert_eq!(crumbs(&document_headings("# A\n## B\n### C\n## D"), 2), "A / B");
    }
}
