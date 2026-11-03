//! Spans of a unit of code from its syntax tree. A unit's next version
//! reparses from its tree, and spans are queried for the ranges asked for,
//! so an edit costs about what it touched.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, Mutex, OnceLock};
use std::task::Poll;

use egui::Context;

use tree_sitter::{InputEdit, Node, Parser, Point, QueryCursor, StreamingIterator as _, Tree};

use super::languages;
use super::{Role, Spans};

thread_local! {
    static PARSER: RefCell<Parser> = RefCell::new(Parser::new());
}

/// A version of a unit of code, parsed, with the spans and injected layers
/// asked of it so far. Spans asked for while a language's queries compile
/// are plain there and kept only once every language is compiled; `ctx`
/// repaints when one is.
pub struct Highlights {
    ctx: Context,
    text: String,
    lines: Lines,
    root: Layer,
    layers: Mutex<Layers>,
    spans: Mutex<HashMap<Range<usize>, Arc<Spans>>>,
}

/// Injected layers by language and the ranges they cover; none where the
/// grammar wouldn't parse them.
type Layers = HashMap<(usize, Vec<Range<usize>>), Option<Arc<Layer>>>;

/// One language's tree over its ranges of the text, with the layers it
/// combines its injections into, found once.
struct Layer {
    lang: usize,
    ranges: Vec<Range<usize>>,
    tree: Tree,
    combined: OnceLock<Vec<(usize, Vec<Range<usize>>)>>,
}

/// Where each line of a text starts.
struct Lines(Vec<usize>);

impl Lines {
    fn of(text: &str) -> Self {
        let starts = text.match_indices('\n').map(|(at, _)| at + 1);
        Self(std::iter::once(0).chain(starts).collect())
    }

    fn point(&self, at: usize) -> Point {
        let row = self.0.partition_point(|start| *start <= at) - 1;
        Point::new(row, at - self.0[row])
    }
}

impl Highlights {
    pub fn new(lang: usize, text: String, ctx: &Context) -> Option<Self> {
        let lines = Lines::of(&text);
        let tree = parse(lang, &text, &lines, &[], None)?;
        Some(Self::of(ctx.clone(), lang, text, lines, tree))
    }

    /// `text` parsed from this version's tree, as the edit between them
    /// left it.
    pub fn edited(&self, text: String) -> Option<Self> {
        let lines = Lines::of(&text);
        let edit = edit_between(&self.text, &self.lines, &text, &lines);
        let mut old = self.root.tree.clone();
        old.edit(&edit);
        let tree = parse(self.root.lang, &text, &lines, &[], Some(&old))?;
        Some(Self::of(self.ctx.clone(), self.root.lang, text, lines, tree))
    }

    fn of(ctx: Context, lang: usize, text: String, lines: Lines, tree: Tree) -> Self {
        let ranges = std::iter::once(0..text.len()).collect();
        let root = Layer { lang, ranges, tree, combined: OnceLock::new() };
        Self { ctx, text, lines, root, layers: Default::default(), spans: Default::default() }
    }

    pub fn ctx(&self) -> &Context {
        &self.ctx
    }

    pub fn lang(&self) -> usize {
        self.root.lang
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn tree(&self) -> &Tree {
        &self.root.tree
    }

    /// The spans within `range`, clipped to it.
    pub fn spans(&self, range: Range<usize>) -> Arc<Spans> {
        if let Some(spans) = self.spans.lock().unwrap().get(&range) {
            return spans.clone();
        }
        let mut roles = vec![Role::Text; range.len()];
        let compiled = self.paint(&self.root, range.clone(), &mut roles);
        let spans = Arc::new(runs(&roles, range.start));
        if compiled {
            self.spans.lock().unwrap().insert(range, spans.clone());
        }
        spans
    }

    /// Paints `layer`'s captures over `region` into `roles`, within its
    /// ranges, then the layers it injects. A node takes its last pattern's
    /// role, and each capture covers those before it, which start no later.
    /// False if a language's queries were still compiling.
    fn paint(&self, layer: &Layer, region: Range<usize>, roles: &mut [Role]) -> bool {
        let queries = match languages::queries(layer.lang, &self.ctx) {
            Poll::Ready(Some(queries)) => queries,
            Poll::Ready(None) => return true,
            Poll::Pending => return false,
        };
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(region.clone());
        let text = self.text.as_bytes();
        let mut captures = cursor.captures(&queries.highlights, layer.tree.root_node(), text);
        let mut found: Vec<(Range<usize>, Option<Role>)> = Vec::new();
        let mut last_node = None;
        while let Some((m, i)) = captures.next() {
            let capture = m.captures()[*i];
            let role = queries.roles[capture.index as usize];
            if last_node == Some(capture.node.id()) {
                if let Some(last) = found.last_mut() {
                    last.1 = role;
                }
                continue;
            }
            last_node = Some(capture.node.id());
            found.push((capture.node.byte_range(), role));
        }
        for (range, role) in found {
            let Some(role) = role else { continue };
            for own in &layer.ranges {
                let start = range.start.max(region.start).max(own.start);
                let end = range.end.min(region.end).min(own.end);
                if start < end {
                    roles[start - region.start..end - region.start].fill(role);
                }
            }
        }

        let mut compiled = true;
        let separate = self.injections(layer, queries, region.clone());
        for (lang, ranges) in separate.iter().chain(self.combined(layer, queries)) {
            if !touch(&extent(ranges), &region) {
                continue;
            }
            if let Some(injected) = self.layer(*lang, ranges) {
                compiled &= self.paint(&injected, region.clone(), roles);
            }
        }
        compiled
    }

    /// `lang`'s layer over `ranges` of this text, parsed once.
    fn layer(&self, lang: usize, ranges: &[Range<usize>]) -> Option<Arc<Layer>> {
        let key = (lang, ranges.to_vec());
        if let Some(layer) = self.layers.lock().unwrap().get(&key) {
            return layer.clone();
        }
        let layer = parse(lang, &self.text, &self.lines, ranges, None).map(|tree| {
            Arc::new(Layer { lang, ranges: key.1.clone(), tree, combined: OnceLock::new() })
        });
        self.layers.lock().unwrap().insert(key, layer.clone());
        layer
    }

    /// The separate layers `layer` injects whose content meets `region`,
    /// each with the ranges it covers: its content node within the layer's
    /// ranges, less its children unless the pattern includes them.
    fn injections(
        &self, layer: &Layer, queries: &languages::Queries, region: Range<usize>,
    ) -> Vec<(usize, Vec<Range<usize>>)> {
        let Some(query) = &queries.injections else { return Vec::new() };
        let mut found = Vec::new();
        let mut cursor = QueryCursor::new();
        cursor.set_byte_range(region.clone());
        let mut matches = cursor.matches(query, layer.tree.root_node(), self.text.as_bytes());
        while let Some(m) = matches.next() {
            if queries.combined[m.pattern_index] {
                continue;
            }
            let Some(injection) = languages::injection(query, m, &self.text) else { continue };
            let Some(lang) = injection.language.and_then(languages::find_token) else { continue };
            if !touch(&injection.content.byte_range(), &region) {
                continue;
            }
            let ranges = intersect(&layer.ranges, injection.content, injection.include_children);
            if !ranges.is_empty() {
                found.push((lang, ranges));
            }
        }
        found
    }

    /// The layers `layer` combines its injections into: per pattern, every
    /// match's content, in the language the last match to name one names.
    fn combined<'a>(
        &self, layer: &'a Layer, queries: &languages::Queries,
    ) -> &'a [(usize, Vec<Range<usize>>)] {
        layer.combined.get_or_init(|| {
            let Some(query) = &queries.injections else { return Vec::new() };
            if !queries.combined.contains(&true) {
                return Vec::new();
            }
            let mut patterns: Vec<(Option<&str>, Vec<Range<usize>>)> = (0..query.pattern_count())
                .map(|_| (None, Vec::new()))
                .collect();
            let mut cursor = QueryCursor::new();
            let text = self.text.as_bytes();
            let mut matches = cursor.matches(query, layer.tree.root_node(), text);
            while let Some(m) = matches.next() {
                if !queries.combined[m.pattern_index] {
                    continue;
                }
                let Some(injection) = languages::injection(query, m, &self.text) else { continue };
                let (language, ranges) = &mut patterns[m.pattern_index];
                if injection.language.is_some() {
                    *language = injection.language;
                }
                ranges.extend(intersect(
                    &layer.ranges,
                    injection.content,
                    injection.include_children,
                ));
            }
            patterns
                .into_iter()
                .filter_map(|(language, mut ranges)| {
                    let lang = languages::find_token(language?)?;
                    ranges.sort_by_key(|range| range.start);
                    (!ranges.is_empty()).then_some((lang, ranges))
                })
                .collect()
        })
    }
}

/// The ranges of `node` inside `parent`, less its children's unless
/// `include_children`.
fn intersect(parent: &[Range<usize>], node: Node, include_children: bool) -> Vec<Range<usize>> {
    let mut own = Vec::new();
    let mut at = node.start_byte();
    if !include_children {
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            own.push(at..child.start_byte());
            at = child.end_byte();
        }
    }
    own.push(at..node.end_byte());
    let mut result = Vec::new();
    for range in own {
        for outer in parent {
            let start = range.start.max(outer.start);
            let end = range.end.min(outer.end);
            if start < end {
                result.push(start..end);
            }
        }
    }
    result
}

/// `lang`'s tree of `ranges` of `text`, or all of it when there are none.
fn parse(
    lang: usize, text: &str, lines: &Lines, ranges: &[Range<usize>], old: Option<&Tree>,
) -> Option<Tree> {
    PARSER.with_borrow_mut(|parser| {
        parser.set_language(&languages::grammar(lang)).ok()?;
        let ranges: Vec<_> = ranges
            .iter()
            .map(|range| tree_sitter::Range {
                start_byte: range.start,
                end_byte: range.end,
                start_point: lines.point(range.start),
                end_point: lines.point(range.end),
            })
            .collect();
        parser.set_included_ranges(&ranges).ok()?;
        parser.parse(text, old)
    })
}

/// The one edit that turns `old` into `new`: what's between their common
/// start and their common end.
fn edit_between(old: &str, old_lines: &Lines, new: &str, new_lines: &Lines) -> InputEdit {
    let (start, end) = agreeing(old, new);
    let (old_end, new_end) = (old.len() - end, new.len() - end);
    InputEdit {
        start_byte: start,
        old_end_byte: old_end,
        new_end_byte: new_end,
        start_position: old_lines.point(start),
        old_end_position: old_lines.point(old_end),
        new_end_position: new_lines.point(new_end),
    }
}

/// How many bytes `a` and `b` agree on at their start, then at their end
/// past that.
pub fn agreeing(a: &str, b: &str) -> (usize, usize) {
    let (a, b) = (a.as_bytes(), b.as_bytes());
    let start = a.iter().zip(b).take_while(|(x, y)| x == y).count();
    let end = a[start..]
        .iter()
        .rev()
        .zip(b[start..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    (start, end)
}

fn extent(ranges: &[Range<usize>]) -> Range<usize> {
    ranges[0].start..ranges[ranges.len() - 1].end
}

/// Whether two ranges overlap or meet.
fn touch(a: &Range<usize>, b: &Range<usize>) -> bool {
    a.start <= b.end && b.start <= a.end
}

/// The runs of `roles` other than text; `roles` starts at `offset`.
fn runs(roles: &[Role], offset: usize) -> Spans {
    let mut spans = Spans::new();
    for (i, role) in roles.iter().enumerate() {
        let at = offset + i;
        if *role == Role::Text {
            continue;
        }
        match spans.last_mut() {
            Some((last, range)) if last == role && range.end == at => range.end = at + 1,
            _ => spans.push((*role, at..at + 1)),
        }
    }
    spans
}
