//! The rows of a front matter block, read from its bytes. The editor draws
//! a property per row and edits the key and value in place; the index
//! reads the same rows for completions. [`parse`] gives bytes into the
//! block text handed in (fences included), so a caller splices by them;
//! the editor [`FrontMatter::map`]s them to its own offsets.

use std::ops::Range;

use saphyr_parser::{Event, Parser, ScalarStyle, Span};

/// YAML's indicator characters: a plain scalar starts with none of them,
/// but for `-`, `?`, and `:` with no space after.
const INDICATORS: &str = "-?:,[]{}#&*!|>'\"%@`";

/// A front matter block: its fences and the rows between them, at offsets
/// of type `O`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FrontMatter<O = usize> {
    /// The opening fence line and its newline.
    pub head: (O, O),
    /// The closing fence line through the end of the block (a trailing
    /// blank line included), or empty when the block has no closing fence.
    pub tail: (O, O),
    /// One per content line or multi-line construct, in order.
    pub rows: Vec<Row<O>>,
}

/// One row of the block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row<O = usize> {
    /// The source line(s) of the row, newline excluded.
    pub line: (O, O),
    pub kind: RowKind<O>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowKind<O = usize> {
    /// `key: value` on one line.
    Property { key: (O, O), sep: (O, O), value: Value<O> },
    /// A line with no colon yet: a key being typed.
    Pending { key: (O, O) },
    /// Nothing on the line.
    Blank,
    /// YAML the grid does not edit in place: comments, nested structure,
    /// lists, multi-line scalars, anchors, tags, and lines that fail to
    /// parse (`error`). Shown as source.
    Source { error: bool },
}

/// The opening and closing quote characters of a quoted scalar.
pub type Quotes<O = usize> = Option<((O, O), (O, O))>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Value<O = usize> {
    /// A one-line scalar; `quotes` are the quote characters around `text`.
    Text { text: (O, O), quotes: Quotes<O> },
    /// No value yet; `at` is where one is typed. `needs_space` when the
    /// line is `key:` with no space after the colon.
    Empty { at: O, needs_space: bool },
}

impl<O: Copy> FrontMatter<O> {
    /// The block's own span: the opening fence through the end of the tail.
    pub fn node(&self) -> (O, O) {
        (self.head.0, self.tail.1)
    }

    /// The same block with every offset passed through `f`.
    pub fn map<P: Copy>(&self, f: impl Fn(O) -> P) -> FrontMatter<P> {
        let r = |(a, b): (O, O)| (f(a), f(b));
        let quotes = |q: Quotes<O>| q.map(|(open, close)| (r(open), r(close)));
        let value = |value: &Value<O>| match value {
            Value::Text { text, quotes: q } => Value::Text { text: r(*text), quotes: quotes(*q) },
            Value::Empty { at, needs_space } => {
                Value::Empty { at: f(*at), needs_space: *needs_space }
            }
        };
        let rows = self.rows.iter().map(|row| {
            let kind = match &row.kind {
                RowKind::Property { key, sep, value: v } => {
                    RowKind::Property { key: r(*key), sep: r(*sep), value: value(v) }
                }
                RowKind::Pending { key } => RowKind::Pending { key: r(*key) },
                RowKind::Blank => RowKind::Blank,
                RowKind::Source { error } => RowKind::Source { error: *error },
            };
            Row { line: r(row.line), kind }
        });
        FrontMatter { head: r(self.head), tail: r(self.tail), rows: rows.collect() }
    }
}

impl<O: Copy + Ord> FrontMatter<O> {
    /// The row whose line holds `offset`, line end included.
    pub fn row_at(&self, offset: O) -> Option<usize> {
        self.rows
            .iter()
            .position(|r| r.line.0 <= offset && offset <= r.line.1)
    }
}

impl<O: Copy + Ord> Row<O> {
    pub fn is_blank(&self) -> bool {
        matches!(self.kind, RowKind::Blank)
    }

    /// Where the caret lands entering the row from the left.
    pub fn entry(&self) -> O {
        match &self.kind {
            RowKind::Property { key, .. } | RowKind::Pending { key } => key.0,
            _ => self.line.0,
        }
    }

    /// The row's cells, as ⌘← and ⌘→ see them: its key, then its value.
    pub fn cells(&self) -> Vec<(O, O)> {
        match &self.kind {
            RowKind::Property { key, value, .. } => vec![*key, (value.caret(), value.end())],
            RowKind::Pending { key } => vec![*key],
            _ => vec![],
        }
    }

    /// Where the caret lands entering the row from the right.
    pub fn exit(&self) -> O {
        match &self.kind {
            RowKind::Property { value, .. } => value.end(),
            RowKind::Pending { key } => key.1,
            RowKind::Blank => self.line.0,
            _ => self.line.1,
        }
    }
}

impl<O: Copy + Ord> Value<O> {
    /// Where the caret lands entering the value.
    pub fn caret(&self) -> O {
        match self {
            Value::Text { text, .. } => text.0,
            Value::Empty { at, .. } => *at,
        }
    }

    pub fn end(&self) -> O {
        match self {
            Value::Text { text, .. } => text.1,
            Value::Empty { at, .. } => *at,
        }
    }
}

/// Reads a block, fences included, as the comrak node holds it. Returns
/// `None` when the text does not open with a fence line.
pub fn parse(block: &str) -> Option<FrontMatter> {
    let lines = line_ranges(block);
    let first = lines.first()?;
    if !is_fence(&block[first.clone()]) {
        return None;
    }
    let head = (first.start, line_end_with_newline(block, first));
    let close = lines
        .iter()
        .skip(1)
        .position(|l| is_fence(&block[l.clone()]))
        .map(|i| i + 1);
    let (content, tail) = match close {
        Some(i) => (1..i, (lines[i].start, block.len())),
        None => (1..lines.len(), (block.len(), block.len())),
    };
    let rows = rows(block, &lines, content);
    Some(FrontMatter { head, tail, rows })
}

fn is_fence(line: &str) -> bool {
    line.trim_end_matches('\r') == "---"
}

/// `(start, end)` of each line, `\n` and `\r\n` excluded; a trailing
/// newline does not start another line.
fn line_ranges(text: &str) -> Vec<Range<usize>> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, b) in text.bytes().enumerate() {
        if b == b'\n' {
            let end = if i > start && text.as_bytes()[i - 1] == b'\r' { i - 1 } else { i };
            out.push(start..end);
            start = i + 1;
        }
    }
    if start < text.len() || out.is_empty() {
        out.push(start..text.len());
    }
    out
}

fn line_end_with_newline(text: &str, line: &Range<usize>) -> usize {
    let rest = &text[line.end..];
    line.end + if rest.starts_with("\r\n") { 2 } else { usize::from(rest.starts_with('\n')) }
}

fn pair(r: &Range<usize>) -> (usize, usize) {
    (r.start, r.end)
}

/// Classifies the `content` lines. The block is parsed whole, its fences
/// blanked; a line that would not parse is blanked too and the rest
/// parsed again, so one bad line costs one source row, not the grid.
fn rows(block: &str, lines: &[Range<usize>], content: Range<usize>) -> Vec<Row> {
    if content.is_empty() {
        return vec![];
    }
    let mut kinds: Vec<Option<RowKind>> = vec![None; lines.len()];
    let mut text = block.to_string();
    for (i, l) in lines.iter().enumerate() {
        let s = block[l.clone()].trim_end_matches('\r');
        if !content.contains(&i) {
            kinds[i] = Some(RowKind::Blank);
            blank(&mut text, l.clone());
        } else if s.trim().is_empty() {
            kinds[i] = Some(RowKind::Blank);
        } else if s.trim_start().starts_with('#') {
            // a construct's end event can sit past a comment after it
            kinds[i] = Some(RowKind::Source { error: false });
        } else if let Some(key) = pending_key(s) {
            kinds[i] = Some(RowKind::Pending { key: (l.start, l.start + key) });
            blank(&mut text, l.clone());
        } else if colon_without_space(s) {
            kinds[i] = Some(RowKind::Source { error: true });
            blank(&mut text, l.clone());
        }
    }

    // parse; blank the line an error points at and parse again
    let mut last_error = usize::MAX;
    let parsed = loop {
        match structure(&text) {
            Ok(p) => break p,
            Err(at) => {
                let at = at.min(text.len().saturating_sub(1));
                let mut i = lines
                    .iter()
                    .position(|l| at < l.end)
                    .unwrap_or(lines.len() - 1)
                    .clamp(content.start, content.end - 1);
                // no progress: the construct started on an earlier line
                if last_error != usize::MAX && at >= last_error {
                    i = kinds[..i].iter().rposition(|k| k.is_none()).unwrap_or(i);
                }
                last_error = at;
                if kinds[i].is_none() {
                    kinds[i] = Some(RowKind::Source { error: true });
                }
                blank(&mut text, lines[i].clone());
                if text.trim().is_empty() {
                    break vec![];
                }
            }
        }
    };

    // every parsed entry spans whole lines; a line nothing parsed is a
    // comment or a construct the grid shows as source
    let mut out = Vec::new();
    let mut i = content.start;
    while i < content.end {
        let l = &lines[i];
        if let Some(kind) = kinds[i].take() {
            out.push(Row { line: pair(l), kind });
            i += 1;
            continue;
        }
        if let Some(entry) = parsed.iter().find(|e| e.first_line == i) {
            // lines classified without the parser after a construct are rows
            // of their own; a blank one inside it is its own
            let mut last = entry.last_line.min(content.end - 1);
            while last > i && kinds[last].is_some() {
                last -= 1;
            }
            let inside = i + 1..=last;
            if inside
                .clone()
                .any(|j| !matches!(kinds[j], None | Some(RowKind::Blank)))
            {
                // the parser saw a line inside blanked, so it read a
                // different construct than the text holds
                for j in i..=last {
                    let kind = kinds[j].take().unwrap_or(RowKind::Source { error: false });
                    out.push(Row { line: pair(&lines[j]), kind });
                }
                i = last + 1;
                continue;
            }
            inside.for_each(|j| kinds[j] = None);
            out.push(Row { line: (l.start, lines[last].end), kind: entry.kind.clone() });
            i = last + 1;
            continue;
        }
        out.push(Row { line: pair(l), kind: RowKind::Source { error: false } });
        i += 1;
    }
    out
}

/// Overwrites a line with spaces so offsets stay put.
fn blank(text: &mut String, line: Range<usize>) {
    let spaces = " ".repeat(line.len());
    text.replace_range(line, &spaces);
}

/// The length of a key being typed: a line at column 0 with no colon that
/// no YAML construct starts with.
fn pending_key(line: &str) -> Option<usize> {
    let first = line.chars().next()?;
    if first.is_whitespace() || line.contains(':') || INDICATORS.contains(first) {
        return None;
    }
    Some(line.trim_end().len())
}

/// `key:value` — a colon with no space after it on a line that would be a
/// key, which YAML reads as one scalar.
fn colon_without_space(line: &str) -> bool {
    let Some(first) = line.chars().next() else { return false };
    if first.is_whitespace() || INDICATORS.contains(first) {
        return false;
    }
    match line.find(':') {
        Some(i) => {
            let rest = &line[i + 1..];
            !(rest.is_empty() || rest.starts_with(' ') || rest.starts_with('\t'))
        }
        None => false,
    }
}

/// A property's separator and value.
type SepValue = ((usize, usize), Value);
struct Entry {
    first_line: usize,
    last_line: usize,
    /// A property or source.
    kind: RowKind,
}

/// Walks the YAML events; `Err(index)` is where parsing failed.
fn structure(text: &str) -> Result<Vec<Entry>, usize> {
    let lines = line_ranges(text);
    // the parser counts chars; everything here is bytes
    let byte_at: Vec<usize> = text
        .char_indices()
        .map(|(b, _)| b)
        .chain(std::iter::once(text.len()))
        .collect();
    let at = |i: usize| byte_at[i.min(byte_at.len() - 1)];
    let span = |s: &Span| at(s.start.index())..at(s.end.index());
    let line_of = |index: usize| line_index(&lines, index);
    let last_line_of = |s: &Span| line_index(&lines, span(s).end.saturating_sub(1));
    let mut events = Vec::new();
    for r in Parser::new_from_str(text) {
        match r {
            Ok(e) => events.push(e),
            Err(e) => return Err(at(e.marker().index())),
        }
    }
    let mut entries = Vec::new();
    let mut it = events.into_iter().peekable();
    // the document must be a mapping at the top; anything else is source
    let mut top_mapping = false;
    while let Some((ev, _)) = it.peek() {
        match ev {
            Event::StreamStart | Event::DocumentStart(_) => {
                it.next();
            }
            Event::MappingStart(0, None) => {
                top_mapping = true;
                it.next();
                break;
            }
            _ => break,
        }
    }
    if !top_mapping {
        // whole thing is one construct; rows() marks uncovered lines source
        return Ok(entries);
    }
    while let Some((key_ev, key_span)) = it.next() {
        let key = span(&key_span);
        let key = match key_ev {
            Event::MappingEnd | Event::DocumentEnd | Event::StreamEnd => break,
            Event::Scalar(_, ScalarStyle::Plain, 0, None)
                if !text[key.clone()].contains('\n')
                    && lines.iter().any(|l| l.start == key.start) =>
            {
                Some(key)
            }
            _ => None,
        };
        let first_line = line_of(at(key_span.start.index()));
        // the value: one event, or a nested construct to skip
        let Some((val_ev, val_span)) = it.next() else { break };
        let mut last_line = last_line_of(&val_span).max(first_line);
        let value = match (&key, &val_ev) {
            (Some(key), Event::Scalar(_, style, 0, None)) => {
                scalar_value(text, &lines, key, *style, span(&val_span))
                    .filter(|(_, value)| rest_of_line_blank(text, &lines, value, last_line))
            }

            _ => None,
        };
        if matches!(val_ev, Event::MappingStart(..) | Event::SequenceStart(..)) {
            let mut depth = 1;
            for (ev, s) in it.by_ref() {
                match ev {
                    Event::SequenceStart(..) | Event::MappingStart(..) => depth += 1,
                    Event::SequenceEnd | Event::MappingEnd => depth -= 1,
                    _ => {}
                }
                last_line = last_line.max(last_line_of(&s));
                if depth == 0 {
                    break;
                }
            }
        }
        let kind = match (key, value) {
            (Some(key), Some((sep, value))) => RowKind::Property { key: pair(&key), sep, value },
            _ => RowKind::Source { error: false },
        };
        entries.push(Entry { first_line, last_line, kind });
    }
    Ok(entries)
}

/// The line holding `index`, its line break included.
fn line_index(lines: &[Range<usize>], index: usize) -> usize {
    lines.iter().rposition(|l| l.start <= index).unwrap_or(0)
}

/// The colon after `key` and the spaces after it.
fn separator(text: &str, lines: &[Range<usize>], key: &Range<usize>) -> Option<(usize, usize)> {
    let line = &lines[line_index(lines, key.start)];
    let colon = key.end + text[key.end..line.end].find(':')?;
    let spaces = text[colon + 1..].len() - text[colon + 1..].trim_start_matches(' ').len();
    Some((colon, colon + 1 + spaces))
}

/// A one-line scalar's quotes and text from its span as written, or `None`
/// for one the grid shows as source (multi-line, block style). A quoted
/// scalar's span runs on through the spaces and comment after it.
fn scalar_text(
    text: &str, style: ScalarStyle, written: Range<usize>,
) -> Option<(Quotes, (usize, usize))> {
    match style {
        ScalarStyle::Plain if !text[written.clone()].contains('\n') => Some((None, pair(&written))),
        ScalarStyle::SingleQuoted | ScalarStyle::DoubleQuoted => {
            let open = written.start;
            let close = closing_quote(text, open)?;
            if text[open..close].contains('\n') {
                return None;
            }
            Some((Some(((open, open + 1), (close, close + 1))), (open + 1, close)))
        }
        _ => None,
    }
}

/// The index of the quote closing the one at `open`; `''` escapes in
/// single quotes, `\"` in double.
fn closing_quote(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let quote = bytes[open];
    let mut i = open + 1;
    while i < bytes.len() {
        let b = bytes[i];
        if quote == b'"' && b == b'\\' {
            i += 2;
            continue;
        }
        if b == quote {
            if quote == b'\'' && bytes.get(i + 1) == Some(&b'\'') {
                i += 2;
                continue;
            }
            return Some(i);
        }
        i += 1;
    }
    None
}

/// The separator and value of a one-line scalar after `key`, or `None`
/// for a scalar the grid shows as source (multi-line, block style).
fn scalar_value(
    text: &str, lines: &[Range<usize>], key: &Range<usize>, style: ScalarStyle,
    written: Range<usize>,
) -> Option<SepValue> {
    let line = &lines[line_index(lines, key.start)];
    let sep = separator(text, lines, key)?;
    if written.is_empty() {
        // `key:` ends the line, or `key: ` does
        let at = line.end.min(sep.1.max(sep.0 + 1));
        let needs_space = sep.1 == sep.0 + 1;
        return Some((sep, Value::Empty { at, needs_space }));
    }
    if written.start < sep.1 {
        return None;
    }
    let (quotes, text) = scalar_text(text, style, written)?;
    Some((sep, Value::Text { text, quotes }))
}

fn rest_of_line_blank(text: &str, lines: &[Range<usize>], value: &Value, last_line: usize) -> bool {
    let end = match value {
        Value::Text { quotes: Some((_, close)), .. } => close.1,
        Value::Text { text: t, quotes: None, .. } => t.1,
        Value::Empty { at, .. } => *at,
    };
    let line = &lines[last_line];
    text[end.min(line.end)..line.end].trim().is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows(block: &str) -> Vec<String> {
        parse(block)
            .unwrap()
            .rows
            .iter()
            .map(|r| match &r.kind {
                RowKind::Property { key, value, .. } => {
                    let v = match value {
                        Value::Text { text, quotes, .. } => format!(
                            "{}{}",
                            &block[text.0..text.1],
                            if quotes.is_some() { " (quoted)" } else { "" }
                        ),
                        Value::Empty { needs_space, .. } => {
                            format!("<empty{}>", if *needs_space { " needs space" } else { "" })
                        }
                    };
                    format!("{} = {}", &block[key.0..key.1], v)
                }
                RowKind::Pending { key } => format!("pending {}", &block[key.0..key.1]),
                RowKind::Blank => "blank".into(),
                RowKind::Source { error } => {
                    format!("source{}", if *error { " (error)" } else { "" })
                }
            })
            .collect()
    }

    /// Each line between the fences is in exactly one row, in order.
    fn rows_tile_the_lines(block: &str, p: &FrontMatter) -> Result<(), String> {
        let mut next = p.head.1;
        for row in &p.rows {
            if row.line.0 != next || row.line.1 < row.line.0 {
                return Err(format!("row {:?} where a row starts at {next}", row.line));
            }
            let rest = &block[row.line.1..];
            next = row.line.1 + if rest.starts_with("\r\n") { 2 } else { 1 };
        }
        if p.tail.0 != p.tail.1 && next != p.tail.0 {
            return Err(format!("rows end at {next}, the closing fence at {}", p.tail.0));
        }
        Ok(())
    }

    /// Rule: each line reads as an editable property, a key being typed, a
    /// blank, or source for what the grid does not edit in place; a
    /// construct's lines, blanks inside included, are one row, unless a
    /// line breaks it, and a bad line costs only its own row.
    #[test]
    fn lines_read_as_properties_or_source() {
        assert!(parse("title: x\n").is_none());
        for (fm, want) in [
            ("---\n\n---\n", &["blank"][..]),
            ("---\ntitle: x\n", &["title = x"]),
            (
                "---\nt: \"Foo: bar\"\nq: 'single'\nn: 42\nempty:\nempty2: \nc: x # note\n---\n",
                &[
                    "t = Foo: bar (quoted)",
                    "q = single (quoted)",
                    "n = 42",
                    "empty = <empty needs space>",
                    "empty2 = <empty>",
                    "source",
                ],
            ),
            ("---\nt: \"Foo\"  # c\n---\n", &["source"]),
            (
                "---\nnested:\n  k: v\nlit: |\n  one\n  two\ndeep:\n  - - a\nafter: y\n---\n",
                &["source", "source", "source", "after = y"],
            ),
            ("---\nt: [a] # c\n---\n", &["source"]),
            ("---\nt:\n  - a # c\n---\n", &["source"]),
            ("---\nt: # c\n  - a\n---\n", &["source"]),
            (
                "---\n# owned by ops\ntitle: x\n\nstatus\nnext: y\n---\n",
                &["source", "title = x", "blank", "pending status", "next = y"],
            ),
            (
                "---\ntitle: \"unclosed\nnext: y\nkey:bar\nlast: z\n---\n",
                &["source (error)", "next = y", "source (error)", "last = z"],
            ),
            (
                "---\nnotes: |\n  one\nurl:http://x\nlast: z\n---\n",
                &["source", "source (error)", "last = z"],
            ),
            ("---\n- a\n- b\n---\n", &["source", "source"]),
        ] {
            assert_eq!(rows(fm), want, "{fm:?}");
            rows_tile_the_lines(fm, &parse(fm).unwrap()).unwrap();
        }
    }
}
