//! Typing in code, inside a fence or in a code file: Enter keeps the
//! indent and adds a level after an open bracket, and a closing bracket
//! typed at a line's start outdents to its match.

use std::sync::Arc;

use comrak::nodes::{AstNode, NodeValue};
use lb_rs::model::text::offset_types::{
    Grapheme, IntoRangeExt as _, RangeExt as _, ToRangeExt as _,
};
use lb_rs::model::text::operation_types::{Operation, Replace};

use crate::syntax::{Highlights, Lang, Role};
use crate::tab::markdown_editor::bounds::RangesExt as _;
use crate::tab::markdown_editor::{MdEdit, MdRender};

/// The code around an offset: one code block's code or a whole code file,
/// as one text.
pub struct CodeScope {
    pub text: String,
    /// Where each code line starts: its grapheme in the buffer and its byte
    /// in `text`, in order.
    pub lines: Vec<(Grapheme, usize)>,
    pub highlights: Option<Arc<Highlights>>,
}

impl CodeScope {
    /// Where the code on `line` starts, if `line` is one of ours.
    pub fn line_start(&self, line: (Grapheme, Grapheme)) -> Option<Grapheme> {
        let idx = self
            .lines
            .partition_point(|(start, _)| *start < line.start());
        self.lines
            .get(idx)
            .map(|(start, _)| *start)
            .filter(|start| *start <= line.end())
    }

    /// What an indent level is here: a tab where lines start with tabs,
    /// else the narrowest indent of two or more spaces up to four, else four.
    pub fn indent_unit(&self) -> String {
        let lines = self.text.lines().filter(|line| !line.trim().is_empty());
        if self.text.lines().any(|line| line.starts_with('\t')) {
            return "\t".into();
        }
        let width = lines
            .map(|line| line.len() - line.trim_start_matches(' ').len())
            .filter(|width| *width >= 2)
            .min()
            .map_or(4, |width| width.min(4));
        " ".repeat(width)
    }

    /// The role of the byte at `at`, read from the window of spans around
    /// it so a scan queries each window once.
    pub fn role_at(&self, at: usize) -> Role {
        const WINDOW: usize = 4096;
        let Some(highlights) = &self.highlights else { return Role::Text };
        let start = at / WINDOW * WINDOW;
        let spans = highlights.spans(start..(start + WINDOW).min(self.text.len()));
        let idx = spans.partition_point(|(_, span)| span.end <= at);
        match spans.get(idx) {
            Some((role, span)) if span.contains(&at) => *role,
            _ => Role::Text,
        }
    }

    /// Whether a bracket at `at` counts, rather than being part of a
    /// string or comment.
    pub fn is_bracket(&self, at: usize) -> bool {
        !matches!(self.role_at(at), Role::String | Role::Comment)
    }

    /// The open bracket a `close` bracket at `at` would pair with.
    pub fn matching_open_before(&self, close: u8, at: usize) -> Option<usize> {
        let open = opener_of(close)?;
        let mut depth = 0;
        for (idx, b) in self.text.as_bytes()[..at].iter().enumerate().rev() {
            if (*b != close && *b != open) || !self.is_bracket(idx) {
                continue;
            }
            if *b == close {
                depth += 1;
            } else if *b == open {
                if depth == 0 {
                    return Some(idx);
                }
                depth -= 1;
            }
        }
        None
    }

    /// The close bracket the open bracket at `at` pairs with.
    pub fn matching_close(&self, at: usize) -> Option<usize> {
        let open = *self.text.as_bytes().get(at)?;
        let close = closer_of(open)?;
        let mut depth = 0;
        for (idx, b) in self.text.as_bytes().iter().enumerate().skip(at + 1) {
            if (*b != close && *b != open) || !self.is_bracket(idx) {
                continue;
            }
            if *b == open {
                depth += 1;
            } else if *b == close {
                if depth == 0 {
                    return Some(idx);
                }
                depth -= 1;
            }
        }
        None
    }
}

pub fn opener_of(close: u8) -> Option<u8> {
    match close {
        b')' => Some(b'('),
        b']' => Some(b'['),
        b'}' => Some(b'{'),
        _ => None,
    }
}

pub fn closer_of(open: u8) -> Option<u8> {
    match open {
        b'(' => Some(b')'),
        b'[' => Some(b']'),
        b'{' => Some(b'}'),
        _ => None,
    }
}

impl<'ast> MdRender {
    /// The code `offset` is in: a whole file's when it isn't a note, or a
    /// code block's when `offset` is on one of its code lines.
    pub fn code_scope_at(&self, root: &'ast AstNode<'ast>, offset: Grapheme) -> Option<CodeScope> {
        if self.plaintext {
            let text = self.buffer.current.text.clone();
            let lines = self
                .bounds
                .source_lines
                .iter()
                .map(|line| (line.start(), self.offset_to_byte(line.start()).0))
                .collect();
            let highlights = Lang::from_extension(&self.ext)
                .and_then(|lang| self.syntax.highlights(lang, text.clone()));
            return Some(CodeScope { text, lines, highlights });
        }
        let (line_idx, _) = self.bounds.source_lines.find_containing(offset, true, true);
        let node = root.descendants().find(|node| {
            matches!(node.data.borrow().value, NodeValue::CodeBlock(_))
                && self.node_first_line_idx(node) <= line_idx
                && line_idx <= self.node_last_line_idx(node)
        })?;
        let NodeValue::CodeBlock(code_block) = &node.data.borrow().value else { return None };
        let (text, lines) = self.code_block_text(node, code_block, false);
        let highlights = Lang::from_info(&code_block.info)
            .and_then(|lang| self.syntax.highlights(lang, text.clone()));
        let scope = CodeScope { text, lines, highlights };
        scope.line_start(self.line_at_offset(offset))?;
        Some(scope)
    }

    /// `offset`'s byte in `scope`'s text, `offset` brought forward to the
    /// code of its line.
    pub fn code_byte(&self, scope: &CodeScope, offset: Grapheme) -> usize {
        let idx = scope
            .lines
            .partition_point(|(start, _)| *start <= offset)
            .saturating_sub(1);
        let (start, at) = scope.lines[idx];
        at + (self.offset_to_byte(offset.max(start)) - self.offset_to_byte(start)).0
    }
}

impl<'ast> MdEdit {
    /// Enter in code: the new line keeps the indent, gains a level after an
    /// open bracket, and between a pair of brackets puts the close on a line
    /// of its own.
    pub fn newline_in_code(
        &self, root: &'ast AstNode<'ast>, operations: &mut Vec<Operation>,
    ) -> bool {
        let selection = self.renderer.buffer.current.selection;
        let Some(scope) = self.renderer.code_scope_at(root, selection.start()) else {
            return false;
        };
        let line = self.renderer.line_at_offset(selection.start());
        let Some(code_start) = scope.line_start(line) else { return false };
        let end_line = self.renderer.line_at_offset(selection.end());
        if scope.line_start(end_line).is_none() {
            return false;
        }
        let (prefix, content) = if self.renderer.plaintext {
            (String::new(), line)
        } else {
            let container = self
                .renderer
                .deepest_container_block_at_offset(root, selection.start());
            let prefix = self
                .renderer
                .extension_prefix(container)
                .unwrap_or_default();
            (prefix, self.renderer.line_content(container, line))
        };
        // the line's leading whitespace, fence indent included, up to the caret
        let content_text = &self.renderer.buffer[content];
        let mut indent =
            content_text[..content_text.len() - content_text.trim_start().len()].to_string();
        let before =
            &self.renderer.buffer[(content.start(), selection.start().max(content.start()))];
        if before.trim().is_empty() {
            indent.truncate(before.len());
        }

        let caret = self
            .renderer
            .code_byte(&scope, selection.start().max(code_start));
        let after = self
            .renderer
            .code_byte(&scope, selection.end().max(code_start));
        let opens = scope.text[..caret]
            .trim_end_matches([' ', '\t'])
            .len()
            .checked_sub(1)
            .filter(|at| closer_of(scope.text.as_bytes()[*at]).is_some() && scope.is_bracket(*at));
        let closes = opens.is_some_and(|open| {
            let rest = scope.text[after..].trim_start_matches([' ', '\t']);
            let at = scope.text.len() - rest.len();
            scope.matching_close(open) == Some(at)
        });

        let mut text = format!("\n{prefix}{indent}");
        if opens.is_some() {
            text += &scope.indent_unit();
        }
        // the caret is placed between the inserts: one inserted at the
        // caret lands after it
        operations.push(Operation::Replace(Replace { range: selection, text }));
        operations.push(Operation::Select(selection.start().to_range()));
        if closes {
            let range = selection.end().into_range();
            operations
                .push(Operation::Replace(Replace { range, text: format!("\n{prefix}{indent}") }));
        }
        true
    }

    /// A close bracket typed where its line has only whitespace before it
    /// lands at the indent of the line its match is on.
    pub fn replace_in_code(
        &self, root: &'ast AstNode<'ast>, range: (Grapheme, Grapheme), text: &str,
        advance_cursor: bool, operations: &mut Vec<Operation>,
    ) -> bool {
        let close = match text.as_bytes() {
            [close] if opener_of(*close).is_some() => *close,
            _ => return false,
        };
        if !range.is_empty() {
            return false;
        }
        let Some(scope) = self.renderer.code_scope_at(root, range.start()) else { return false };
        let line = self.renderer.line_at_offset(range.start());
        let Some(code_start) = scope.line_start(line) else { return false };
        if code_start > range.start()
            || !self.renderer.buffer[(code_start, range.start())]
                .trim()
                .is_empty()
        {
            return false;
        }
        let at = self.renderer.code_byte(&scope, range.start());
        // a line of a string or comment, the byte before it its line break
        if !scope.is_bracket(at.saturating_sub(1)) {
            return false;
        }
        let Some(open) = scope.matching_open_before(close, at) else { return false };
        let open_line = scope.text[..open]
            .rfind('\n')
            .map_or(0, |newline| newline + 1);
        let open_code = &scope.text[open_line..];
        let indent =
            &open_code[..open_code.len() - open_code.trim_start_matches([' ', '\t']).len()];
        let range = (code_start, range.start());
        operations.push(Operation::Replace(Replace { range, text: format!("{indent}{text}") }));
        if advance_cursor {
            operations.push(Operation::Select(range.end().to_range()));
        }
        true
    }
}
