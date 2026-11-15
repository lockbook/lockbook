//! The words a diff pairs between two versions of a text. Merges keep or drop whole words, so a word is what an edit
//! from the other side can't land inside of without changing what it is.

use comrak::nodes::{AstNode, NodeValue};
use comrak::{Arena, Options};
use unicode_segmentation::UnicodeSegmentation as _;

use super::offset_types::Byte;

/// How a text splits into words.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Words {
    /// Unicode's word boundaries.
    #[default]
    Unicode,
    /// Unicode's word boundaries, but markdown's images, links' targets, wiki links, code spans, math, autolinks,
    /// shortcodes, inline HTML and escapes are each one word, and an emphasis-like run's delimiters are words of
    /// their own.
    Markdown,
}

impl Words {
    /// Where `text`'s words start, then its end.
    pub fn bounds(self, text: &str) -> Vec<Byte> {
        let unicode = text.split_word_bound_indices().map(|(i, _)| i);
        let mut bounds: Vec<usize> = match self {
            Self::Unicode => unicode.collect(),
            Self::Markdown => {
                let Syntax { mut atoms, cuts } = markdown(text);
                atoms.sort_unstable();
                let mut bounds: Vec<usize> = unicode
                    .chain(atoms.iter().flat_map(|a| [a.0, a.1]))
                    .chain(cuts)
                    .filter(|&i| i < text.len())
                    .collect();
                bounds.sort_unstable();
                bounds.dedup();
                // no bound inside an atom; both are in order, so one sweep finds them
                let mut atom = atoms.iter().peekable();
                let mut end = 0;
                bounds.retain(|&i| {
                    while let Some(a) = atom.next_if(|a| a.0 < i) {
                        end = end.max(a.1);
                    }
                    i >= end
                });
                // a grapheme is never split: one the syntax ends inside, such as `)` with a combining mark, joins the
                // word before it
                let graphemes: Vec<usize> = text.grapheme_indices(true).map(|(i, _)| i).collect();
                bounds.retain(|i| graphemes.binary_search(i).is_ok());
                bounds
            }
        };
        bounds.push(text.len());
        bounds.into_iter().map(Byte).collect()
    }
}

/// The options markdown is parsed with, in the editor and for merges alike.
pub fn markdown_options() -> Options<'static> {
    let mut options = Options::default();
    options.parse.ignore_setext = true;
    options.extension.alerts = true;
    options.extension.autolink = true;
    options.extension.description_lists = false; // todo: is this a good way to power workspace-wide term definitions?
    options.extension.footnotes = false;
    options.extension.front_matter_delimiter = Some("---".to_string());
    options.extension.greentext = false;
    options.extension.header_id_prefix = None; // intended for HTML renderers
    options.extension.highlight = true;
    options.extension.math_code = true; // rendered as code for now
    options.extension.math_dollars = true; // rendered as code for now
    options.extension.multiline_block_quotes = false; // todo
    options.extension.shortcodes = true;
    options.extension.spoiler = true;
    options.extension.strikethrough = true;
    options.extension.subscript = true;
    options.extension.superscript = true;
    options.extension.table = true;
    options.extension.tasklist = true;
    options.extension.underline = true;
    options.extension.wikilinks_title_after_pipe = true; // matches obsidian
    options.extension.wikilinks_title_before_pipe = false; // would not match obsidian
    options.render.escaped_char_spans = true;
    options
}

/// Byte ranges of a markdown text's syntax: `atoms` no word boundary falls inside, and `cuts` where one falls.
struct Syntax {
    atoms: Vec<(usize, usize)>,
    cuts: Vec<usize>,
}

fn markdown(text: &str) -> Syntax {
    // parsed as the editor parses it
    let source = format!("{text}\n");
    let line_starts: Vec<usize> = [0]
        .into_iter()
        .chain(source.match_indices('\n').map(|(i, _)| i + 1))
        .collect();
    let arena = Arena::new();
    let root = comrak::parse_document(&arena, &source, &markdown_options());
    let range = |node: &AstNode| {
        let pos = node.data.borrow().sourcepos;
        let byte = |line: usize, column: usize| {
            line_starts
                .get(line.saturating_sub(1))
                .map_or(source.len(), |start| start + column)
        };
        // sourcepos is inclusive, 1-based
        (
            byte(pos.start.line, pos.start.column.saturating_sub(1)),
            byte(pos.end.line, pos.end.column),
        )
    };
    let mut syntax = Syntax { atoms: vec![], cuts: vec![] };
    for node in root.descendants() {
        let value = &node.data.borrow().value;
        let whole = range(node);
        match value {
            NodeValue::Image(_)
            | NodeValue::WikiLink(_)
            | NodeValue::Code(_)
            | NodeValue::Math(_)
            | NodeValue::ShortCode(_)
            | NodeValue::HtmlInline(_)
            | NodeValue::Escaped => syntax.atoms.push(whole),
            // an autolink is its address
            NodeValue::Link(_) if source.as_bytes().get(whole.0) != Some(&b'[') => {
                syntax.atoms.push(whole)
            }
            // a link's text is words, its target one word
            NodeValue::Link(_) => {
                let text_end = node.last_child().map_or(whole.0 + 1, |c| range(c).1);
                syntax.atoms.push((text_end, whole.1));
                syntax.cuts.extend([whole.0, whole.0 + 1]);
            }
            NodeValue::Emph
            | NodeValue::Strong
            | NodeValue::Strikethrough
            | NodeValue::Highlight
            | NodeValue::Insert
            | NodeValue::Underline
            | NodeValue::Superscript
            | NodeValue::Subscript
            | NodeValue::SpoileredText => {
                let (first, last) = (node.first_child(), node.last_child());
                syntax.cuts.extend([whole.0, whole.1]);
                syntax.cuts.extend(first.map(|c| range(c).0));
                syntax.cuts.extend(last.map(|c| range(c).1));
            }
            _ => {}
        }
    }
    syntax.atoms.retain(|a| a.0 < a.1);
    syntax
}

#[cfg(test)]
mod test {
    use super::Words;

    fn words(text: &str) -> Vec<&str> {
        let bounds = Words::Markdown.bounds(text);
        bounds.windows(2).map(|w| &text[w[0].0..w[1].0]).collect()
    }

    #[test]
    fn an_image_is_one_word() {
        assert_eq!(words("x ![a b](a.png) y"), ["x", " ", "![a b](a.png)", " ", "y"]);
    }

    #[test]
    fn a_links_text_is_words_and_its_target_one() {
        assert_eq!(words("[a b](u.test/x)"), ["[", "a", " ", "b", "](u.test/x)"]);
    }

    #[test]
    fn an_emphasis_delimiter_is_its_own_word() {
        assert_eq!(words("_xxxx_ y"), ["_", "xxxx", "_", " ", "y"]);
    }

    #[test]
    fn a_literal_underscore_stays_in_its_word() {
        assert_eq!(words("a_b"), ["a_b"]);
    }

    #[test]
    fn code_spans_wiki_links_and_autolinks_are_one_word() {
        assert_eq!(
            words("`a b` [[c d]] https://x.test/a/b"),
            ["`a b`", " ", "[[c d]]", " ", "https://x.test/a/b"]
        );
    }
}
