//! What another device's edit does to the view. A remote version lands at
//! the top of a frame (`Editor::reload`); the text on screen
//! should stay where it was unless the edit touched it.

use std::mem::Discriminant;

use comrak::nodes::NodeValue;
use egui::{Event as Egui, Modifiers, MouseWheelSource, MouseWheelUnit, Pos2, TouchPhase};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use unicode_segmentation::UnicodeSegmentation as _;

use super::super::super::input::{Event, Location, Region};
use super::super::super::widget::utils::wrap_layout::FragmentContent;
use super::super::doc_gen::{Features, gen_doc, gen_table};
use super::super::harness::TestEditor;
use super::{IMAGE, Prop, lb};
use crate::test_utils::byte_source::ByteSource;

/// The harness's screen.
pub(super) const VIEW: egui::Vec2 = egui::vec2(800.0, 600.0);
/// Bytes per generated block of a tall note.
pub(super) const BLOCK: usize = 64;
/// Enough blocks to scroll a few screens.
pub(super) const BLOCKS: usize = 16;

/// A node's kind and range.
type Node = (Discriminant<NodeValue>, (Grapheme, Grapheme));
/// The nodes that hold a fragment and its innermost block.
type Holders = (Vec<Node>, Option<(Grapheme, Grapheme)>);

/// The view as versions start landing, for [`Prop::TextStays`] and
/// [`Prop::CaretStays`].
pub(super) struct Before {
    text: String,
    view: (Grapheme, Grapheme),
    seen: Vec<Seen>,
    /// For each of `seen` under [`Prop::TextStays`], the nodes that hold
    /// it and its innermost block.
    nodes: Vec<Holders>,
    caret: Option<f32>,
    selection: Grapheme,
}

impl Before {
    pub(super) fn new(ws: &mut TestEditor, prop: Prop) -> Option<Self> {
        let view = view_range(ws)?;
        let seen = visible(ws);
        let nodes = if prop == Prop::TextStays {
            seen.iter()
                .map(|s| (nodes_at(ws, s.source_range.0), leaf_block(ws, s.source_range.0)))
                .collect()
        } else {
            vec![]
        };
        let text = ws.get_text().to_string();
        let selection = ws.editor.edit.renderer.buffer.current.selection.1;
        Some(Self { text, view, seen, nodes, caret: caret_y(ws), selection })
    }

    /// Once the versions landed and the view settled: text on screen stays
    /// put under an edit wholly above or below it, and a caret on screen
    /// stays on screen. Above counts only once `scrolled`: unscrolled, the
    /// markup before the first fragment drawn isn't above the view.
    pub(super) fn judge(
        self, ws: &mut TestEditor, prop: Prop, scrolled: bool, log: &mut dyn FnMut(String),
    ) -> Result<(), String> {
        let text = ws.get_text().to_string();
        let changes = lb_rs::model::text::diff(&self.text, &text);
        let Some(a) = changes.iter().map(|r| r.range.start().0).min() else { return Ok(()) };
        let b = changes.iter().map(|r| r.range.end().0).max().unwrap_or(a);
        let shift =
            text.graphemes(true).count() as isize - self.text.graphemes(true).count() as isize;
        let (lo, hi) = self.view;
        let above = scrolled && b <= lo.0;
        if !above && a < hi.0 {
            log(format!("the versions reach the view at {a}..{b}"));
            return Ok(());
        }
        let edit = if above { "above" } else { "below" };
        log(format!("the versions edit {a}..{b}, {edit} the view's {lo:?}..{hi:?}"));
        // the caret holds where it is drawn, so text past an edit at or after it may move
        let held = self.caret.is_some() && self.selection.0 <= a;
        // a node's edge before the edit, where it is after: gone if the edit replaced it
        let moved = |g: Grapheme| match g.0 {
            g if g < a => Some(Grapheme(g)),
            g if g >= b => Some(Grapheme((g as isize + shift) as usize)),
            _ => None,
        };
        let shifted = |r: (Grapheme, Grapheme)| {
            let by = if r.0.0 >= b { shift } else { 0 };
            (Grapheme((r.0.0 as isize + by) as usize), Grapheme((r.1.0 as isize + by) as usize))
        };
        match prop {
            Prop::TextStays => {
                let judged: Vec<(&Seen, &Holders)> = self
                    .seen
                    .iter()
                    .zip(&self.nodes)
                    .filter(|(s, _)| !held || s.source_range.1.0 <= a)
                    .collect();
                // text whose nodes the edit reparsed may move by design; only the rest must stay
                let reparsed = judged.iter().any(|(s, (nodes, _))| {
                    let was: Vec<_> = nodes
                        .iter()
                        .map(|&(kind, (start, end))| (kind, moved(start), moved(end)))
                        .collect();
                    let now: Vec<_> = nodes_at(ws, shifted(s.source_range).0)
                        .into_iter()
                        .map(|(kind, (start, end))| (kind, Some(start), Some(end)))
                        .collect();
                    now != was
                });
                if reparsed {
                    log("the edit changed how the text in view parses".to_string());
                    return Ok(());
                }
                log(format!("checked {} fragments in view", judged.len()));
                let after = laid_out(ws);
                // each fragment's move, or None if it isn't drawn as it was, and
                // whether any of its text is drawn
                let mut moves = vec![];
                let mut drawn = vec![];
                for (seen, _) in &judged {
                    let now = after.iter().find(|s| {
                        s.source_range == shifted(seen.source_range) && s.text == seen.text
                    });
                    moves.push(now.map(|now| now.top - seen.top));
                    drawn.push(after.iter().any(|s| {
                        s.source_range
                            .intersects(&shifted(seen.source_range), false)
                    }));
                }
                // a block the edit re-wrapped moves its pieces apart, and what
                // follows it with them; otherwise the text moves as one
                let rewrapped = (0..judged.len()).any(|i| {
                    let block = (0..judged.len()).filter(|&j| judged[j].1.1 == judged[i].1.1);
                    block.into_iter().any(|j| match (moves[i], moves[j]) {
                        (Some(a), Some(b)) => (a - b).abs() > 0.5,
                        (None, _) => drawn[i],
                        _ => false,
                    })
                });
                if rewrapped {
                    log("the edit re-wrapped text in view".to_string());
                    return Ok(());
                }
                for ((seen, _), by) in judged.iter().zip(moves) {
                    let Some(by) = by else {
                        return Err(format!(
                            "{:?} at y {} left the view on an edit {edit}",
                            seen.text, seen.top
                        ));
                    };
                    if by.abs() > 0.5 {
                        return Err(format!(
                            "{:?} moved from y {} to {} on an edit {edit}",
                            seen.text,
                            seen.top,
                            seen.top + by,
                        ));
                    }
                }
            }
            _ => {
                if let (Some(before), after) = (self.caret, caret_y(ws)) {
                    match after {
                        Some(y) if (0.0..=VIEW.y).contains(&y) => {}
                        _ => return Err(format!("the caret at y {before} went to {after:?}")),
                    }
                }
            }
        }
        Ok(())
    }
}

/// A text fragment drawn wholly inside the view: its source range, text and
/// top.
#[derive(Debug)]
pub(super) struct Seen {
    pub(super) source_range: (Grapheme, Grapheme),
    text: String,
    top: f32,
    bottom: f32,
}

impl Seen {
    pub(super) fn len(&self) -> usize {
        self.source_range.len().0
    }
}

fn visible(ws: &TestEditor) -> Vec<Seen> {
    let on_screen = |s: &Seen| s.top >= 0.0 && s.bottom <= VIEW.y && s.bottom > s.top;
    laid_out(ws).into_iter().filter(on_screen).collect()
}

/// Every text fragment laid out, on screen or not, but spaces alone: a
/// line's break can swallow one.
fn laid_out(ws: &TestEditor) -> Vec<Seen> {
    let text = ws.get_text();
    let segs = &ws.editor.edit.renderer.buffer.current.segs;
    ws.editor
        .edit
        .renderer
        .fragments
        .iter()
        .filter(|f| matches!(f.content, FragmentContent::Glyphs { .. }))
        .filter(|f| !f.source_range.is_empty())
        .map(|f| {
            let (a, b) = segs.range_to_byte(f.source_range);
            Seen {
                source_range: f.source_range,
                text: text[a.0..b.0].to_string(),
                top: f.rect.top(),
                bottom: f.rect.bottom(),
            }
        })
        .filter(|s| !s.text.trim().is_empty())
        .collect()
}

pub(super) fn pick_visible(ws: &TestEditor, src: &mut ByteSource) -> Option<Seen> {
    let mut seen = visible(ws);
    (!seen.is_empty()).then(|| seen.swap_remove(src.draw(seen.len())))
}

/// The lowest and highest offsets drawn in the view, even in part.
pub(super) fn view_range(ws: &TestEditor) -> Option<(Grapheme, Grapheme)> {
    let frags = ws.editor.edit.renderer.fragments.iter();
    let shown: Vec<_> = frags
        .filter(|f| f.rect.bottom() > 0.0 && f.rect.top() < VIEW.y)
        .collect();
    let lo = shown.iter().map(|f| f.source_range.start()).min()?;
    let hi = shown.iter().map(|f| f.source_range.end()).max()?;
    Some((lo, hi))
}

/// The nodes that hold `at`, outermost first: their kinds and ranges.
fn nodes_at(ws: &mut TestEditor, at: Grapheme) -> Vec<Node> {
    let arena = crate::tab::markdown_editor::Arena::new();
    let r = &mut ws.editor.edit.renderer;
    let mut node = r.reparse(&arena);
    let mut kinds = vec![];
    while let Some(child) = node.children().find(|n| {
        let (start, end) = r.node_range(n);
        start <= at && at <= end
    }) {
        kinds.push((std::mem::discriminant(&child.data.borrow().value), r.node_range(child)));
        node = child;
    }
    kinds
}

/// The range of the innermost block that holds `at`.
fn leaf_block(ws: &mut TestEditor, at: Grapheme) -> Option<(Grapheme, Grapheme)> {
    let arena = crate::tab::markdown_editor::Arena::new();
    let r = &mut ws.editor.edit.renderer;
    let mut node = r.reparse(&arena);
    let mut block = None;
    while let Some(child) = node.children().find(|n| {
        let (start, end) = r.node_range(n);
        start <= at && at <= end
    }) {
        if child.data.borrow().value.block() {
            block = Some(r.node_range(child));
        }
        node = child;
    }
    block
}

/// The caret's top on screen, if it's drawn in the view.
fn caret_y(ws: &TestEditor) -> Option<f32> {
    let caret = ws.editor.edit.renderer.buffer.current.selection.1;
    let [top, _] = ws.editor.edit.cursor_line(caret)?;
    (0.0..=VIEW.y).contains(&top.y).then_some(top.y)
}

/// Frames half a second apart until the scroll stops moving: a wheel's
/// glide and heights measured late both play out over time.
pub(super) fn settle(ws: &mut TestEditor) {
    let mut last = None;
    for _ in 0..20 {
        ws.enter_frame_with_input_after(0.5, vec![]);
        let now = ws.editor.edit.scroll_area.stored_offset();
        if now.is_some() && now == last {
            return;
        }
        last = now;
    }
}

pub(super) fn scroll(ws: &mut TestEditor, by: f32) {
    let center = Pos2::new(VIEW.x / 2.0, VIEW.y / 2.0);
    let mut left = by;
    while left > 0.0 {
        let delta = left.min(100.0);
        left -= delta;
        ws.enter_frame_with_input(vec![
            Egui::PointerMoved(center),
            Egui::MouseWheel {
                unit: MouseWheelUnit::Point,
                delta: egui::vec2(0.0, -delta),
                modifiers: Modifiers::NONE,
                phase: TouchPhase::Move,
                source: MouseWheelSource::Wheel,
            },
        ]);
    }
}

/// Generated blocks with images and tables among them, each from its own
/// [`BLOCK`] bytes, until the bytes run out.
pub(super) fn long_doc(bytes: &[u8], features: &Features) -> String {
    let blocks: Vec<String> = bytes
        .chunks(BLOCK)
        .map(|chunk| {
            let src = &mut ByteSource::new(chunk);
            match src.bias(&[6, 1, 1]) {
                0 => gen_doc(src, features).trim_end().to_string(),
                1 => format!("![a]({IMAGE})"),
                _ => gen_table(src, features).trim_end().to_string(),
            }
        })
        .collect();
    blocks.join("\n\n") + "\n"
}

/// Edits above the view that keep the text on screen in place: a taller
/// first block, a paragraph added at the top, and an edit above the view
/// in a list or code block that straddles its top.
#[test]
fn reloads_above_the_view() {
    let open = |md: &str| {
        let (mut ws, _embeds) = TestEditor::with_test_embeds(lb(), md);
        settle(&mut ws);
        scroll(&mut ws, 1000.0);
        settle(&mut ws);
        ws
    };
    let land = |ws: &mut TestEditor, next: String| {
        ws.editor.reload(next, Some([1; 32]));
        settle(ws);
    };
    let top = |ws: &TestEditor| visible(ws).into_iter().next().map(|s| (s.text, s.top));
    let paras = (0..120)
        .map(|i| format!("p{i}"))
        .collect::<Vec<_>>()
        .join("\n\n")
        + "\n";
    let wrapping = "and some more words ".repeat(12);

    // as many blocks, a taller first one: the text stays
    let mut ws = open(&paras);
    let before = top(&ws);
    let next = ws
        .get_text()
        .replacen("p0\n", &format!("p0 {wrapping}\n"), 1);
    land(&mut ws, next);
    assert_eq!(top(&ws), before);

    // a paragraph added at the top
    let mut ws = open(&paras);
    let before = top(&ws);
    let next = format!("inserted\n\n{}", ws.get_text());
    land(&mut ws, next);
    assert_eq!(top(&ws), before);

    // a long list straddles the view's top, and an item above the view wraps
    let list = (0..120)
        .map(|i| format!("- item{i}"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    let mut ws = open(&list);
    let before = top(&ws);
    let next = ws
        .get_text()
        .replacen("- item1\n", &format!("- item1 {wrapping}\n"), 1);
    land(&mut ws, next);
    assert_eq!(top(&ws), before);

    // a long code block straddles the view's top, and loses a line above it
    let code = format!("```\n{}```\n", (0..120).map(|i| format!("line{i}\n")).collect::<String>());
    let mut ws = open(&code);
    let before = top(&ws);
    let next = ws.get_text().replacen("line1\n", "", 1);
    land(&mut ws, next);
    assert_eq!(top(&ws), before);

    // a paragraph added at the top as the user scrolls: the text moves by the scroll
    let mut ws = open(&paras);
    let seen = visible(&ws);
    let (text, y) = (seen[seen.len() / 2].text.clone(), seen[seen.len() / 2].top);
    let next = format!("inserted\n\n{}", ws.get_text());
    ws.editor.reload(next, Some([1; 32]));
    scroll(&mut ws, 50.0);
    settle(&mut ws);
    let now = visible(&ws)
        .into_iter()
        .find(|s| s.text == text)
        .map(|s| s.top);
    assert!(now.is_some_and(|now| (now - (y - 50.0)).abs() <= 0.5), "{now:?} for {}", y - 50.0);
}

/// A paragraph added above the view leaves the text where it was drawn.
#[test]
fn a_paragraph_added_above_the_view_leaves_the_text_in_place() {
    let paras = (0..120)
        .map(|i| format!("p{i}"))
        .collect::<Vec<_>>()
        .join("\n\n")
        + "\n";
    let mut ws = TestEditor::with_lb(lb(), &paras);
    settle(&mut ws);
    scroll(&mut ws, 1000.0);
    settle(&mut ws);
    let top = |ws: &TestEditor| visible(ws).into_iter().next().map(|s| (s.text, s.top));
    let before = top(&ws);
    let next = format!("inserted\n\n{}", ws.get_text());
    ws.editor.reload(next, Some([1; 32]));
    settle(&mut ws);
    assert_eq!(top(&ws), before);
}

/// A version landing between the reading line and the caret, and beside
/// the caret, leaves the caret where it was drawn.
#[test]
fn a_version_landing_above_the_caret_leaves_it_in_place() {
    let paras = (0..120)
        .map(|i| format!("p{i}"))
        .collect::<Vec<_>>()
        .join("\n\n")
        + "\n";
    let mut ws = TestEditor::with_lb(lb(), &paras);
    settle(&mut ws);
    scroll(&mut ws, 1000.0);
    settle(&mut ws);
    let seen = visible(&ws);
    let line = seen.len() * 3 / 4;
    let caret = Grapheme(seen[line].source_range.0.0 + 1);
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(caret),
            end: Location::Grapheme(caret),
        },
    });
    settle(&mut ws);
    let y = caret_y(&ws);
    assert!(y.is_some());

    // paragraphs added above the line before the caret's, and the letter
    // before the caret replaced
    let (above, at) = (&seen[line - 1].text, &seen[line].text);
    let added = "added\n\n".repeat(20);
    let next = ws
        .get_text()
        .replacen(&format!("\n{above}\n"), &format!("\n{added}{above}\n"), 1)
        .replacen(&format!("\n{at}\n"), &format!("\nZ{}\n", &at[1..]), 1);
    ws.editor.reload(next, Some([1; 32]));
    settle(&mut ws);
    assert_eq!(caret_y(&ws), y);
}

/// The spaces before a block's first line lay out in the block's row.
#[test]
fn a_blocks_indent_is_in_its_row() {
    use super::super::super::scroll_content::{DocRowId, DocScrollContent};
    let mut ws = TestEditor::with_lb(lb(), "p0\n\n  p1\n");
    let arena = super::super::super::Arena::new();
    let root = ws.editor.edit.renderer.reparse(&arena);
    let rows = DocScrollContent::for_frame(&ws.editor.edit.renderer, root, VIEW.y);
    assert_eq!(rows.find_text_row(Grapheme(4)), Some(DocRowId::Block(1)));
}
