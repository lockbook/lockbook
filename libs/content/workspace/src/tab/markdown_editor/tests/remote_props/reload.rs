//! A note another program keeps rewriting while it's open. Each new version
//! on disk reaches the editor through `Editor::reload` at the top of a
//! frame, as a finished load does; the task manager may collapse several
//! into the latest. Nothing here saves or undoes: from [`Events::Typing`]
//! the reader types one letter between versions.

use lb_rs::model::text::offset_types::Grapheme;
use unicode_segmentation::UnicodeSegmentation as _;

use super::super::super::input::{Event, Location, Region};
use super::super::doc_gen::{Features, gen_doc};
use super::super::harness::TestEditor;
use super::{Events, lb};
use crate::test_utils::byte_source::ByteSource;

/// What a reader types between versions; nothing on disk has it.
const TYPED: char = 'Ж';

/// After every frame the note shows the latest version, merged from
/// [`Events::Typing`] with what was typed: edits apart from each other both
/// land, inserts at the same spot in either order, and where they overlap
/// the note keeps its own.
pub(super) fn run(features: Features, events: Events, seeds: u64) {
    assert!(
        (Events::Reading..=Events::Typing).contains(&events),
        "only versions and typing land here"
    );
    super::run(seeds, 512, |b, log| check(b, &features, events, log));
}

fn check(
    buf: &[u8], features: &Features, events: Events, log: &mut dyn FnMut(String),
) -> Result<(), String> {
    // the run's choices come first: docs may use up the rest
    let (choices, docs) = buf.split_at(buf.len().min(96));
    let mut src = ByteSource::new(choices);
    let mut docs = ByteSource::new(docs);
    let mut disk = gen_doc(&mut docs, features);
    log(format!("open {disk:?}"));
    let mut ws = TestEditor::with_lb(lb(), &disk);
    let (mut landed, mut want) = (disk.clone(), disk.clone());
    let mut seq = 0u8;
    for _ in 0..1 + src.draw(8) {
        // the agent writes one or more versions before the next frame
        let versions = 1 + src.bias(&[4, 2, 1]);
        let delivered = src.bias(&[3, 1, 1]);
        for v in 0..versions {
            disk = next_version(&disk, &mut src, &mut docs, features, log);
            // 0: each lands; 1: all land in one frame; 2: collapsed to the last
            if delivered != 2 || v + 1 == versions {
                seq = seq.wrapping_add(1);
                let before = ws.get_text().to_string();
                ws.editor.reload(disk.clone(), Some([seq; 32]));
                let merges = merges(&landed, &before, &disk);
                if !merges.iter().any(|m| m == ws.get_text()) {
                    return fail(ws.get_text(), &merges, log);
                }
                (landed, want) = (disk.clone(), ws.get_text().to_string());
            }
            if delivered == 0 {
                ws.enter_frame();
                expect(&ws, &want, log)?;
            }
        }
        if delivered != 0 {
            log(format!(
                "{} versions land in one frame",
                if delivered == 1 { versions } else { 1 }
            ));
            ws.enter_frame();
            expect(&ws, &want, log)?;
        }
        if events >= Events::Typing && src.bias(&[1, 1]) == 1 {
            let len = ws
                .editor
                .edit
                .renderer
                .buffer
                .current
                .segs
                .last_cursor_position()
                .0;
            let at = Grapheme(src.draw(len + 1));
            log(format!("type {TYPED:?} at {at:?}"));
            ws.push(Event::Replace {
                region: Region::BetweenLocations {
                    start: Location::Grapheme(at),
                    end: Location::Grapheme(at),
                },
                text: TYPED.to_string(),
                advance_cursor: true,
            });
            ws.enter_frame();
            want = ws.get_text().to_string();
        }
    }
    let (_, saved) = ws.editor.flush_for_save();
    log(format!("a save would write {saved:?}"));
    Ok(())
}

fn expect(ws: &TestEditor, want: &str, log: &mut dyn FnMut(String)) -> Result<(), String> {
    if ws.get_text() == want {
        return Ok(());
    }
    fail(ws.get_text(), &[want.to_string()], log)
}

fn fail(shown: &str, merges: &[String], log: &mut dyn FnMut(String)) -> Result<(), String> {
    log(format!("shows {shown:?}"));
    Err(format!("the note shows {shown:?}, it should show one of {merges:?}"))
}

/// Every text a merge of `local` and `remote`, both edits of `base`, may give.
/// Disjoint edits both apply, an insert at a replaced range's ends lies outside
/// it, inserts at the same spot land in either order, and a remote edit that
/// overlaps a local one is dropped.
fn merges(base: &str, local: &str, remote: &str) -> Vec<String> {
    use lb_rs::model::text::diff;
    use lb_rs::model::text::offset_types::RangeExt as _;
    let segs = lb_rs::model::text::unicode_segs::calc(base);
    let overlap =
        |a: (Grapheme, Grapheme), b: (Grapheme, Grapheme)| match (a.is_empty(), b.is_empty()) {
            (false, false) => a.0 < b.1 && b.0 < a.1,
            (true, false) => b.0 < a.0 && a.0 < b.1,
            (false, true) => a.0 < b.0 && b.0 < a.1,
            (true, true) => false,
        };
    let local = diff(base, local);
    let mut edits: Vec<_> = diff(base, remote)
        .into_iter()
        .filter(|r| local.iter().all(|l| !overlap(l.range, r.range)))
        .collect();
    edits.extend(local);
    edits.sort_by_key(|e| (e.range.0, !e.range.is_empty()));

    // each tie between two inserts doubles the orders
    let mut orders = vec![edits];
    for i in 1..orders[0].len() {
        let (a, b) = (&orders[0][i - 1].range, &orders[0][i].range);
        if a.is_empty() && b.is_empty() && a.0 == b.0 {
            let swapped: Vec<_> = orders
                .iter()
                .map(|o| {
                    let mut o = o.clone();
                    o.swap(i - 1, i);
                    o
                })
                .collect();
            orders.extend(swapped);
        }
    }
    orders
        .into_iter()
        .map(|edits| {
            let (mut out, mut at) = (String::new(), 0);
            for e in edits {
                let (start, end) = segs.range_to_byte(e.range);
                out.push_str(&base[at..start.0]);
                out.push_str(&e.text);
                at = end.0;
            }
            out + &base[at..]
        })
        .collect()
}

/// The agent's next version of `disk`.
fn next_version(
    disk: &str, src: &mut ByteSource, docs: &mut ByteSource, features: &Features,
    log: &mut dyn FnMut(String),
) -> String {
    let bounds: Vec<usize> = disk
        .grapheme_indices(true)
        .map(|(i, _)| i)
        .chain([disk.len()])
        .collect();
    let at = |src: &mut ByteSource| bounds[src.draw(bounds.len())];
    let lines: Vec<&str> = disk.split_inclusive('\n').collect();
    let next = match src.bias(&[4, 2, 2, 2, 1, 1]) {
        // a word added
        0 => {
            let a = at(src);
            let word = ["foo ", "bar ", "the ", "- [ ] ", "\n", "hello world "][src.draw(6)];
            format!("{}{word}{}", &disk[..a], &disk[a..])
        }
        // a range deleted
        1 => {
            let (a, b) = (at(src), at(src));
            let (a, b) = (a.min(b), a.max(b));
            format!("{}{}", &disk[..a], &disk[b..])
        }
        // a line repeated somewhere else
        2 if !lines.is_empty() => {
            let line = lines[src.draw(lines.len())];
            let to = src.draw(lines.len() + 1);
            let mut out: Vec<&str> = lines.clone();
            out.insert(to, line);
            out.concat()
        }
        // two lines swapped
        3 if lines.len() > 1 => {
            let mut out: Vec<&str> = lines.clone();
            let (i, j) = (src.draw(out.len()), src.draw(out.len()));
            out.swap(i, j);
            out.concat()
        }
        // the whole file rewritten
        4 => gen_doc(docs, features),
        // the whole file rewritten, keeping the same words in a new order
        _ => {
            let mut words: Vec<&str> = disk.split_word_bounds().collect();
            let by = src.draw(words.len().max(1)).min(words.len());
            words.rotate_left(by);
            words.concat()
        }
    };
    log(format!("disk becomes {next:?}"));
    next
}

/// Typing at the start of a note whose versions repeat one word: the typed
/// letter stays in front of every word the versions add.
#[test]
fn a_letter_typed_at_the_start_stays_there_through_reloads() {
    use lb_rs::model::text::buffer::Buffer;
    use lb_rs::model::text::operation_types::{Operation, Replace};

    let mut b = Buffer::from("foo\n");
    for v in ["foo foo\n", "foo foo foo\n"] {
        b.reload(v.into());
        b.update();
    }
    b.queue(vec![Operation::Replace(Replace {
        range: (Grapheme(0), Grapheme(0)),
        text: "Ж".into(),
    })]);
    b.update();
    for v in ["foo foo foo foo\n", "foo foo foo foo foo\n"] {
        b.reload(v.into());
        b.update();
    }
    assert_eq!(b.current.text, "Жfoo foo foo foo foo\n");
}
