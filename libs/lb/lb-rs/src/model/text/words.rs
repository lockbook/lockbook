//! The words a diff pairs between two versions of a text. Merges keep or drop whole words, so a word is what an edit
//! from the other side can't land inside of without changing what it is.

use comrak::Options;
use unicode_segmentation::UnicodeSegmentation as _;

use super::offset_types::Byte;

/// How a text splits into words.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Words {
    /// Unicode's word boundaries.
    #[default]
    Unicode,
    /// Markdown's words.
    Markdown,
}

impl Words {
    /// Where `text`'s words start, then its end.
    pub fn bounds(self, text: &str) -> Vec<Byte> {
        let unicode = text.split_word_bound_indices().map(|(i, _)| i);
        // markdown's syntax doesn't decide its words yet
        let mut bounds: Vec<usize> = unicode.collect();
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
