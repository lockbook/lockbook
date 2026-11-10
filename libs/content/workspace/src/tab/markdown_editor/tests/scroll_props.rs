//! Property tests over what another device's edit does to the view. A
//! remote version lands at the top of a frame (`Editor::apply_disk_version`);
//! the text on screen should stay where it was unless the edit touched it.

use egui::{Event as Egui, Modifiers, MouseWheelSource, MouseWheelUnit, Pos2, TouchPhase};
use lb_rs::model::text::offset_types::{Grapheme, RangeExt as _};
use unicode_segmentation::UnicodeSegmentation as _;

use super::super::input::{Event, Location, Region};
use super::super::widget::utils::wrap_layout::FragmentContent;
use super::doc_gen::{Features, gen_doc, gen_table};
use super::harness::{TestEditor, build_lb};
use crate::test_utils::byte_source::ByteSource;

const IMAGE: &str = "https://x.test/a.png";
/// The harness's screen.
const VIEW: egui::Vec2 = egui::vec2(800.0, 600.0);

#[derive(Clone, Copy, PartialEq, Debug)]
enum Prop {
    /// Text on screen stays put under an edit wholly above or below it.
    TextStays,
    /// [`Prop::TextStays`] for edits that leave the note with as many
    /// blocks: the scroll keeps its place by block index, so an edit that
    /// adds or removes a block above moves the text by design.
    TextStaysSameBlocks,
    /// A caret on screen stays on screen.
    CaretStays,
    /// [`Prop::CaretStays`] for edits that leave as many blocks.
    CaretStaysSameBlocks,
}

impl Prop {
    fn same_blocks(self) -> bool {
        matches!(self, Prop::TextStaysSameBlocks | Prop::CaretStaysSameBlocks)
    }

    fn caret(self) -> bool {
        matches!(self, Prop::CaretStays | Prop::CaretStaysSameBlocks)
    }
}

#[test]
#[ignore = "a block added or removed above the view moves the text by its height"]
fn remote_edit_off_screen_leaves_the_text_in_place() {
    run(Prop::TextStays, 300);
}

#[test]
#[ignore = "an edit in the block straddling the view's top, above the view, moves the text"]
fn remote_edit_off_screen_with_as_many_blocks_leaves_the_text_in_place() {
    run(Prop::TextStaysSameBlocks, 300);
}

#[test]
#[ignore = "a block added above the view can push the caret off it"]
fn remote_edit_leaves_the_caret_on_screen() {
    run(Prop::CaretStays, 300);
}

#[test]
#[ignore = "an edit in the block straddling the view's top can push the caret off it"]
fn remote_edit_with_as_many_blocks_leaves_the_caret_on_screen() {
    run(Prop::CaretStaysSameBlocks, 300);
}

fn run(prop: Prop, seeds: u64) {
    crate::test_utils::prop::run(
        seeds,
        512,
        |b| check(b, prop, &mut |_| {}),
        |b| {
            let mut log = vec![];
            let result = check(b, prop, &mut |line| log.push(line));
            format!("{}\n=> {result:?}", log.join("\n"))
        },
    );
}

fn check(buf: &[u8], prop: Prop, log: &mut dyn FnMut(String)) -> Result<(), String> {
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        // the run's choices come first: the doc may use up the rest
        let (choices, doc) = buf.split_at(buf.len().min(64));
        let mut src = ByteSource::new(choices);
        let md = long_doc(&mut ByteSource::new(doc));
        log(format!("doc ({} bytes) {md:?}", md.len()));
        let (mut ws, embeds) = TestEditor::with_test_embeds(build_lb(), &md);
        embeds.complete(IMAGE, egui::vec2(300.0, 200.0));
        settle(&mut ws, prop);
        let wheel = 200.0 * (1 + src.draw(12)) as f32;
        log(format!("scroll down {wheel}pt"));
        scroll(&mut ws, wheel);
        settle(&mut ws, prop);
        if prop.caret() {
            let Some(at) = pick_visible(&ws, &mut src) else { return Ok(()) };
            let caret = Grapheme(at.source_range.start().0 + src.draw(at.len().max(1)));
            log(format!("caret at {caret:?}"));
            ws.push(Event::Select {
                region: Region::BetweenLocations {
                    start: Location::Grapheme(caret),
                    end: Location::Grapheme(caret),
                },
            });
            settle(&mut ws, prop);
        }
        for _ in 0..1 + src.draw(4) {
            step(&mut ws, &mut src, prop, log)?;
        }
        Ok(())
    }));
    caught.unwrap_or_else(|panic| {
        let message = panic.downcast_ref::<String>().map(String::as_str);
        let message = message.or(panic.downcast_ref::<&str>().copied());
        Err(format!("panicked: {}", message.unwrap_or("with no message")))
    })
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
    ws: &mut TestEditor, src: &mut ByteSource, prop: Prop, log: &mut dyn FnMut(String),
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
    // an edit wholly above the view, or wholly below it
    let above = src.bias(&[1, 1]) == 0;
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
    let shift = next.graphemes(true).count() as isize - text.graphemes(true).count() as isize;

    let before = visible(ws);
    let caret = caret_y(ws);
    let blocks = block_count(ws);
    ws.editor.apply_disk_version(next, Some([1; 32]));
    settle(ws, prop);
    let after = visible(ws);
    if prop.same_blocks() && block_count(ws) != blocks {
        log(format!("the edit changed how many blocks there are: {blocks} -> {}", block_count(ws)));
        return Ok(());
    }

    log(format!("checked {} fragments in view", before.len()));
    match prop {
        Prop::TextStays | Prop::TextStaysSameBlocks => {
            let shifted = |r: (Grapheme, Grapheme)| {
                let by = if above { shift } else { 0 };
                (Grapheme((r.0.0 as isize + by) as usize), Grapheme((r.1.0 as isize + by) as usize))
            };
            for seen in &before {
                let now = after
                    .iter()
                    .find(|s| s.source_range == shifted(seen.source_range) && s.text == seen.text);
                match now {
                    Some(now) if (now.top - seen.top).abs() <= 0.5 => {}
                    Some(now) => {
                        return Err(format!(
                            "{:?} moved from y {} to {} on an edit {}",
                            seen.text,
                            seen.top,
                            now.top,
                            if above { "above" } else { "below" },
                        ));
                    }
                    None => {
                        return Err(format!(
                            "{:?} at y {} left the view on an edit {}",
                            seen.text,
                            seen.top,
                            if above { "above" } else { "below" },
                        ));
                    }
                }
            }
        }
        Prop::CaretStays | Prop::CaretStaysSameBlocks => {
            if let (Some(before), after) = (caret, caret_y(ws)) {
                match after {
                    Some(y) if (0.0..=VIEW.y).contains(&y) => {}
                    _ => return Err(format!("the caret at y {before} went to {after:?}")),
                }
            }
        }
    }
    Ok(())
}

fn block_count(ws: &mut TestEditor) -> usize {
    let arena = comrak::Arena::new();
    ws.editor.edit.renderer.reparse(&arena).children().count()
}

/// The caret's top on screen, if it's drawn in the view.
fn caret_y(ws: &TestEditor) -> Option<f32> {
    let caret = ws.editor.edit.renderer.buffer.current.selection.1;
    let [top, _] = ws.editor.edit.cursor_line(caret)?;
    (0.0..=VIEW.y).contains(&top.y).then_some(top.y)
}

/// Frames half a second apart until the scroll stops moving: a wheel's
/// glide and heights measured late both play out over time.
fn settle(ws: &mut TestEditor, prop: Prop) {
    if prop.caret() {
        ws.enter_frame();
    }
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

/// Generated blocks with images and tables among them, long enough to
/// scroll a few screens.
fn long_doc(src: &mut ByteSource) -> String {
    let mut blocks: Vec<String> = vec![];
    while blocks.iter().map(String::len).sum::<usize>() < 2500 {
        match src.bias(&[6, 1, 1]) {
            0 => blocks.push(gen_doc(src, &Features::all()).trim_end().to_string()),
            1 => blocks.push(format!("![a]({IMAGE})")),
            _ => blocks.push(gen_table(src, &Features::all()).trim_end().to_string()),
        }
        if blocks.len() > 200 {
            break;
        }
    }
    blocks.join("\n\n") + "\n"
}

/// The two ways an edit above the view moves the text on master, and the
/// one it doesn't.
#[test]
#[ignore = "a block added above, or an edit above the view in the block straddling its top, moves the text"]
fn remote_edits_above_the_view() {
    let open = |md: &str| {
        let (mut ws, _embeds) = TestEditor::with_test_embeds(build_lb(), md);
        settle(&mut ws, Prop::TextStays);
        scroll(&mut ws, 1000.0);
        settle(&mut ws, Prop::TextStays);
        ws
    };
    let land = |ws: &mut TestEditor, next: String| {
        ws.editor.apply_disk_version(next, Some([1; 32]));
        settle(ws, Prop::TextStays);
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
}
