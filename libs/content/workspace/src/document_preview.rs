//! Stateless document-card excerpts using the editor's Markdown parser and cursor coordinates.

use comrak::{Arena, nodes::NodeValue, parse_document};
use lb_rs::model::text::{
    offset_types::{Byte, Grapheme},
    unicode_segs,
};
use serde::Serialize;

use crate::tab::markdown_editor::MdRender;

const PREVIEW_LINES: usize = 10;
const PREVIEW_GRAPHEMES: usize = 1400;
const LEADING_GRAPHEMES: usize = 350;

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkdownExcerpt {
    pub markdown: String,
    pub focus_line: String,
}

pub fn markdown_excerpt(text: &str, cursor: usize) -> MarkdownExcerpt {
    if text.trim().is_empty() {
        return MarkdownExcerpt::default();
    }
    let segs = unicode_segs::calc(text);
    let cursor = cursor.min(segs.last_cursor_position().0);
    let cursor_byte = segs.offset_to_byte(Grapheme(cursor)).0;
    let starts: Vec<usize> = std::iter::once(0)
        .chain(text.match_indices('\n').map(|(offset, _)| offset + 1))
        .collect();
    let current = starts.partition_point(|&offset| offset <= cursor_byte) - 1;
    let mut first = current.saturating_sub(2);
    let last = (first + PREVIEW_LINES).min(starts.len());
    if last == starts.len() {
        first = last.saturating_sub(PREVIEW_LINES);
    }
    let mut start = starts[first];
    let mut end = starts.get(last).copied().unwrap_or(text.len());
    let first_grapheme = segs.offset_to_char(Byte(start)).0;
    let last_grapheme = segs.offset_to_char(Byte(end)).0;
    if last_grapheme - first_grapheme > PREVIEW_GRAPHEMES {
        let from = first_grapheme.max(cursor.saturating_sub(LEADING_GRAPHEMES));
        start = segs.offset_to_byte(Grapheme(from)).0;
        end = segs
            .offset_to_byte(Grapheme((from + PREVIEW_GRAPHEMES).min(last_grapheme)))
            .0;
    }
    first = starts.partition_point(|&offset| offset <= start) - 1;
    let line_end = |line: usize| starts.get(line + 1).copied().unwrap_or(text.len());
    let focus = (first..=current)
        .rev()
        .find(|&line| !text[starts[line]..line_end(line)].trim().is_empty())
        .unwrap_or(current);
    let focus_start = start.max(starts[focus]);
    let focus_end = end.min(line_end(focus));
    let focus_line = text[focus_start..focus_end.max(focus_start)]
        .trim()
        .to_owned();

    let options = MdRender::comrak_options();
    let mut markdown = String::new();
    // Only inspect the full document when preceding lines might contain an opening fence.
    if first > 0 {
        let arena = Arena::new();
        let root = parse_document(&arena, text, &options);
        for node in root.descendants() {
            let data = node.data.borrow();
            if let NodeValue::CodeBlock(code) = &data.value {
                let pos = data.sourcepos;
                if code.fenced && pos.start.line <= first && first < pos.end.line {
                    // Preserve the original opener, including list/blockquote indentation.
                    let opening = pos.start.line - 1;
                    markdown.push_str(&text[starts[opening]..line_end(opening)]);
                    break;
                }
            }
        }
    }
    // Markdown parsers close a fenced code block at EOF, so no closing-fence repair is needed.
    markdown.push_str(text[start..end].trim_matches(['\n', '\r']));
    MarkdownExcerpt { markdown, focus_line }
}
