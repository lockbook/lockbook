//! Property tests over a note another program keeps rewriting while it's
//! open. Each new version on disk reaches the editor through
//! `Editor::apply_disk_version` at the top of a frame, as a finished load
//! does; the task manager may collapse several into the latest.

use lb_rs::model::text::offset_types::Grapheme;
use unicode_segmentation::UnicodeSegmentation as _;

use super::super::input::{Event, Location, Region};
use super::doc_gen::{Features, gen_doc};
use super::harness::{TestEditor, build_lb};
use crate::test_utils::byte_source::ByteSource;

/// What a reader types between versions; nothing on disk has it.
const TYPED: char = 'Ж';

#[derive(Clone, Copy, PartialEq, Debug)]
enum Prop {
    /// Reading only: after every frame the note shows the latest version.
    ShowsDisk,
    /// Typing a little between versions: the note shows the latest version
    /// plus what was typed.
    ShowsDiskAndTyping,
}

#[test]
#[ignore = "the word diff misplaces an insert after an emoji"]
fn reading_shows_the_latest_version() {
    run(Prop::ShowsDisk, 300);
}

#[test]
#[ignore = "a second reload over unsaved edits lands at stale offsets"]
fn typing_keeps_the_latest_version_and_the_typing() {
    run(Prop::ShowsDiskAndTyping, 300);
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
        // the run's choices come first: docs may use up the rest
        let (choices, docs) = buf.split_at(buf.len().min(96));
        let mut src = ByteSource::new(choices);
        let mut docs = ByteSource::new(docs);
        let mut disk = gen_doc(&mut docs, &Features::all());
        log(format!("open {disk:?}"));
        let mut ws = TestEditor::with_lb(build_lb(), &disk);
        let mut seq = 0u8;
        for _ in 0..1 + src.draw(8) {
            // the agent writes one or more versions before the next frame
            let versions = 1 + src.bias(&[4, 2, 1]);
            let delivered = src.bias(&[3, 1, 1]);
            for v in 0..versions {
                disk = next_version(&disk, &mut src, &mut docs, log);
                // 0: each lands; 1: all land in one frame; 2: collapsed to the last
                if delivered != 2 || v + 1 == versions {
                    seq = seq.wrapping_add(1);
                    ws.editor.apply_disk_version(disk.clone(), Some([seq; 32]));
                }
                if delivered == 0 {
                    ws.enter_frame();
                    expect(&ws, &disk, prop, log)?;
                }
            }
            if delivered != 0 {
                log(format!(
                    "{} versions land in one frame",
                    if delivered == 1 { versions } else { 1 }
                ));
                ws.enter_frame();
                expect(&ws, &disk, prop, log)?;
            }
            if prop == Prop::ShowsDiskAndTyping && src.bias(&[1, 1]) == 1 {
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
            }
        }
        let (_, saved) = ws.editor.flush_for_save();
        log(format!("a save would write {saved:?}"));
        Ok(())
    }));
    caught.unwrap_or_else(|panic| {
        let message = panic.downcast_ref::<String>().map(String::as_str);
        let message = message.or(panic.downcast_ref::<&str>().copied());
        Err(format!("panicked: {}", message.unwrap_or("with no message")))
    })
}

fn expect(
    ws: &TestEditor, disk: &str, prop: Prop, log: &mut dyn FnMut(String),
) -> Result<(), String> {
    let shown = ws.get_text();
    let without_typing: String = shown.chars().filter(|c| *c != TYPED).collect();
    let ok = match prop {
        Prop::ShowsDisk => shown == disk,
        Prop::ShowsDiskAndTyping => without_typing == disk,
    };
    if ok {
        return Ok(());
    }
    log(format!("shows {shown:?}"));
    Err(format!("the note shows {shown:?}, the latest version is {disk:?}"))
}

/// The agent's next version of `disk`.
fn next_version(
    disk: &str, src: &mut ByteSource, docs: &mut ByteSource, log: &mut dyn FnMut(String),
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
        4 => gen_doc(docs, &Features::all()),
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

/// The shortest case behind each ignored row, against the buffer alone.
#[test]
#[ignore = "the word diff misplaces an insert after an emoji, and a second reload over unsaved edits lands early"]
fn reload_cases() {
    use lb_rs::model::text::buffer::Buffer;
    use lb_rs::model::text::operation_types::{Operation, Replace};

    // reading only: one new version
    let mut b = Buffer::from("foo 👋 foo\n");
    b.reload("foo 👋foo  foo\n".into());
    b.update();
    assert_eq!(b.current.text, "foo 👋foo  foo\n");

    // typing one letter between versions
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
