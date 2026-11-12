//! What another device's edit does to the view. A remote version lands at
//! the top of a frame (`Editor::reload`); the text on screen
//! should stay where it was unless the edit touched it.

use egui::{Event as Egui, Modifiers, MouseWheelSource, MouseWheelUnit, Pos2, TouchPhase};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use unicode_segmentation::UnicodeSegmentation as _;

use super::super::super::input::{Event, Location, Region};
use super::super::super::widget::utils::wrap_layout::FragmentContent;
use super::super::doc_gen::{Features, gen_doc, gen_table};
use super::super::harness::TestEditor;
use super::{Events, IMAGE, lb};
use crate::test_utils::byte_source::ByteSource;

/// The harness's screen.
const VIEW: egui::Vec2 = egui::vec2(800.0, 600.0);
/// The run's choices, ahead of the bytes the note is made from.
const CHOICES: usize = 64;
/// Bytes per generated block of the note.
const BLOCK: usize = 64;
/// Enough blocks to scroll a few screens.
const BLOCKS: usize = 16;

/// A caret on screen stays on screen as remote versions land on a long
/// note. Between them the reader types at the caret from [`Events::Typing`]
/// and undoes from [`Events::Undo`]; the caret is placed in view from
/// [`Events::Held`] and the view scrolled from [`Events::Scrolled`].
pub(super) fn run(features: Features, events: Events, seeds: u64) {
    assert!(events >= Events::Reading, "every step lands a remote version");
    super::run(seeds, CHOICES + BLOCK * BLOCKS, |b, log| check(b, &features, events, log));
}

fn check(
    buf: &[u8], features: &Features, events: Events, log: &mut dyn FnMut(String),
) -> Result<(), String> {
    let (choices, doc) = buf.split_at(buf.len().min(CHOICES));
    let mut src = ByteSource::new(choices);
    let md = long_doc(doc, features);
    log(format!("doc ({} bytes) {md:?}", md.len()));
    let (mut ws, embeds) = TestEditor::with_test_embeds(lb(), &md);
    embeds.complete(IMAGE, egui::vec2(300.0, 200.0));
    settle(&mut ws);
    let wheel = 200.0 * (1 + src.draw(12)) as f32;
    if events >= Events::Scrolled {
        log(format!("scroll down {wheel}pt"));
        scroll(&mut ws, wheel);
        settle(&mut ws);
    }
    if events >= Events::Held {
        let Some(at) = pick_visible(&ws, &mut src) else { return Ok(()) };
        let caret = Grapheme(at.source_range.start().0 + src.draw(at.len().max(1)));
        log(format!("caret at {caret:?}"));
        ws.push(Event::Select {
            region: Region::BetweenLocations {
                start: Location::Grapheme(caret),
                end: Location::Grapheme(caret),
            },
        });
        settle(&mut ws);
    }
    for _ in 0..1 + src.draw(4) {
        if events >= Events::Typing && src.bias(&[1, 1]) == 1 {
            let at = ws.editor.edit.renderer.buffer.current.selection.1;
            log(format!("type at {at:?}"));
            ws.push(Event::Replace {
                region: Region::BetweenLocations {
                    start: Location::Grapheme(at),
                    end: Location::Grapheme(at),
                },
                text: "x".into(),
                advance_cursor: true,
            });
            settle(&mut ws);
        }
        if events >= Events::Undo && src.bias(&[2, 1]) == 1 {
            log("undo".into());
            ws.push(Event::Undo);
            settle(&mut ws);
        }
        step(&mut ws, &mut src, events, log)?;
    }
    Ok(())
}

/// A text fragment drawn wholly inside the view: its source range, text and
/// top.
#[derive(Debug)]
struct Seen {
    source_range: (Grapheme, Grapheme),
    text: String,
    top: f32,
}

impl Seen {
    fn len(&self) -> usize {
        self.source_range.len().0
    }
}

fn visible(ws: &TestEditor) -> Vec<Seen> {
    let text = ws.get_text();
    let segs = &ws.editor.edit.renderer.buffer.current.segs;
    ws.editor
        .edit
        .renderer
        .fragments
        .iter()
        .filter(|f| matches!(f.content, FragmentContent::Glyphs { .. }))
        .filter(|f| f.rect.top() >= 0.0 && f.rect.bottom() <= VIEW.y && f.rect.height() > 0.0)
        .filter(|f| !f.source_range.is_empty())
        .map(|f| {
            let (a, b) = segs.range_to_byte(f.source_range);
            Seen {
                source_range: f.source_range,
                text: text[a.0..b.0].to_string(),
                top: f.rect.top(),
            }
        })
        .collect()
}

fn pick_visible(ws: &TestEditor, src: &mut ByteSource) -> Option<Seen> {
    let mut seen = visible(ws);
    (!seen.is_empty()).then(|| seen.swap_remove(src.draw(seen.len())))
}

/// The lowest and highest offsets drawn in the view, even in part.
fn view_range(ws: &TestEditor) -> Option<(Grapheme, Grapheme)> {
    let frags = ws.editor.edit.renderer.fragments.iter();
    let shown: Vec<_> = frags
        .filter(|f| f.rect.bottom() > 0.0 && f.rect.top() < VIEW.y)
        .collect();
    let lo = shown.iter().map(|f| f.source_range.start()).min()?;
    let hi = shown.iter().map(|f| f.source_range.end()).max()?;
    Some((lo, hi))
}

fn step(
    ws: &mut TestEditor, src: &mut ByteSource, events: Events, log: &mut dyn FnMut(String),
) -> Result<(), String> {
    let Some((lo, hi)) = view_range(ws) else { return Ok(()) };
    let text = ws.get_text().to_string();
    let bounds: Vec<usize> = text
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .chain([text.len()])
        .collect();
    let last = bounds.len() - 1;
    log(format!("view covers {lo:?}..{hi:?} of {last}"));
    // unscrolled, the markup before the first fragment drawn isn't above the view
    let above = events >= Events::Scrolled && src.bias(&[1, 1]) == 0;
    let (span_lo, span_hi) = if above { (0, lo.0) } else { (hi.0, last) };
    if span_hi <= span_lo {
        return Ok(());
    }
    let a = span_lo + src.draw(span_hi - span_lo + 1);
    let b = (a + src.draw(40)).min(span_hi);
    let insert = match src.bias(&[3, 2, 2, 1]) {
        0 => "\n\nanother device wrote this paragraph\n\n".to_string(),
        1 => " and some words".to_string(),
        2 => String::new(),
        _ => format!("\n\n![a]({IMAGE})\n\n"),
    };
    let (a_byte, b_byte) = (bounds[a], bounds[b]);
    let mut next = text.clone();
    next.replace_range(a_byte..b_byte, &insert);
    log(format!(
        "remote replaces {:?} at {a} ({}) with {insert:?}",
        &text[a_byte..b_byte],
        if above { "above" } else { "below" }
    ));
    let caret = caret_y(ws);
    ws.editor.reload(next, Some([1; 32]));
    settle(ws);
    if let (Some(before), after) = (caret, caret_y(ws)) {
        match after {
            Some(y) if (0.0..=VIEW.y).contains(&y) => {}
            _ => return Err(format!("the caret at y {before} went to {after:?}")),
        }
    }
    Ok(())
}

/// The caret's top on screen, if it's drawn in the view.
fn caret_y(ws: &TestEditor) -> Option<f32> {
    let caret = ws.editor.edit.renderer.buffer.current.selection.1;
    let [top, _] = ws.editor.edit.cursor_line(caret)?;
    (0.0..=VIEW.y).contains(&top.y).then_some(top.y)
}

/// Frames half a second apart until the scroll stops moving: a wheel's
/// glide and heights measured late both play out over time.
fn settle(ws: &mut TestEditor) {
    ws.enter_frame();
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

fn scroll(ws: &mut TestEditor, by: f32) {
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
fn long_doc(bytes: &[u8], features: &Features) -> String {
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

/// The scroll keeps its place by block index, so a block added above the
/// view moves the text by that block's height.
#[test]
#[ignore = "the scroll anchor is a block index"]
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
