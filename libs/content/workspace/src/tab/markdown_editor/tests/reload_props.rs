//! Property tests over a note another program keeps rewriting while it's
//! open. Each new version on disk reaches the editor through
//! `Editor::reload` at the top of a frame, as a finished load
//! does; the task manager may collapse several into the latest.

use lb_rs::model::text::offset_types::Grapheme;
use unicode_segmentation::UnicodeSegmentation as _;

use super::super::input::{Event, Location, Region};
use super::doc_gen::{Features, gen_doc};
use super::harness::{TestEditor, in_memory_core};
use crate::test_utils::byte_source::ByteSource;

/// What a reader types between versions; nothing on disk has it.
const TYPED: char = 'Ж';

#[derive(Clone, Copy, PartialEq, Debug)]
enum Prop {
    /// Reading only: after every frame the note shows the latest version.
    ShowsDisk,
    /// Typing, deleting, undoing or saving a little between versions: each
    /// version lands as a merge with the note's edits. Edits apart from each
    /// other both land, the note's first where both insert; where they overlap
    /// the note keeps its own.
    ShowsDiskAndTyping,
}

#[test]
fn reading_shows_the_latest_version() {
    run(Prop::ShowsDisk, 300);
}

#[test]
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
        let mut ws = TestEditor::with_lb(in_memory_core(), &disk);
        let mut model = Model::new(&disk);
        let mut want = disk.clone();
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
                    ws.editor.reload(disk.clone(), Some([seq; 32]));
                    want = model.reload(&disk);
                    expect(&ws, &want, log)?;
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
                let at = src.draw(len + 1);
                let replace = |end: usize, text: &str| Event::Replace {
                    region: Region::BetweenLocations {
                        start: Location::Grapheme(Grapheme(at)),
                        end: Location::Grapheme(Grapheme(end)),
                    },
                    text: text.into(),
                    advance_cursor: true,
                };
                match src.bias(&[4, 2, 1, 1]) {
                    0 => {
                        log(format!("type {TYPED:?} at {at}"));
                        ws.push(replace(at, &TYPED.to_string()));
                    }
                    1 if at < len => {
                        log(format!("delete grapheme {at}"));
                        ws.push(replace(at + 1, ""));
                    }
                    2 => {
                        log("undo".into());
                        ws.push(Event::Undo);
                    }
                    _ => {
                        // the save lands, and the agent's next version starts from it
                        let (_, text) = ws.editor.flush_for_save();
                        seq = seq.wrapping_add(1);
                        ws.editor.saved([seq; 32], text.clone());
                        log(format!("save {text:?}"));
                        model = Model::new(&text);
                        disk = text;
                    }
                }
                ws.enter_frame();
                want = ws.get_text().to_string();
                model.edited(&want);
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

fn expect(ws: &TestEditor, want: &str, log: &mut dyn FnMut(String)) -> Result<(), String> {
    if ws.get_text() == want {
        return Ok(());
    }
    fail(ws.get_text(), want, log)
}

fn fail(shown: &str, want: &str, log: &mut dyn FnMut(String)) -> Result<(), String> {
    log(format!("shows {shown:?}"));
    Err(format!("the note shows {shown:?}, it should show {want:?}"))
}

/// What the note should show: runs of the version it last loaded, and between
/// them the note's own text, typed or kept over the disk's. Each run of own
/// text is one local edit of `landed`. A new version merges edit by edit:
/// disjoint edits both apply, an insert at a replaced range's ends lies
/// outside it, a remote insert at a local one's spot lands after it, and a
/// remote edit that overlaps a local one is dropped, the text under it
/// becoming the note's own.
struct Model {
    landed: String,
    spans: Vec<Span>,
}

#[derive(Clone, Debug)]
enum Span {
    /// Bytes of `landed`.
    Disk(usize, usize),
    Own(String),
}

impl Model {
    fn new(disk: &str) -> Self {
        let len = disk.len();
        Self { landed: disk.into(), spans: vec![Span::Disk(0, len), Span::Disk(len, len)] }
    }

    fn shown(&self) -> String {
        self.spans
            .iter()
            .map(|span| match span {
                Span::Disk(a, b) => &self.landed[*a..*b],
                Span::Own(text) => text,
            })
            .collect()
    }

    /// The shown text becomes `after` by one edit, which goes before any run
    /// that starts where it does.
    fn edited(&mut self, after: &str) {
        let before = self.shown();
        if before == after {
            return;
        }
        let mut start = before
            .bytes()
            .zip(after.bytes())
            .take_while(|(a, b)| a == b)
            .count();
        while !before.is_char_boundary(start) {
            start -= 1;
        }
        let same_end =
            |n: usize| before.as_bytes()[before.len() - n..] == after.as_bytes()[after.len() - n..];
        let mut tail = (0..=before.len().min(after.len()) - start)
            .rev()
            .find(|n| same_end(*n))
            .unwrap();
        while !before.is_char_boundary(before.len() - tail) {
            tail -= 1;
        }
        let end = before.len() - tail;
        let text = &after[start..after.len() - tail];

        let cut = |span: &Span, x: usize, y: usize| match span {
            Span::Disk(a, _) => Span::Disk(a + x, a + y),
            Span::Own(text) => Span::Own(text[x..y].into()),
        };
        let (mut left, mut right, mut offset, mut anchored) = (vec![], vec![], 0, false);
        for span in &self.spans {
            let len = match span {
                Span::Disk(a, b) => b - a,
                Span::Own(text) => text.len(),
            };
            let (lo, hi) = (offset, offset + len);
            offset = hi;
            if lo < start {
                left.push(cut(span, 0, hi.min(start) - lo));
            }
            // the edit's start stays a run of disk, if empty, so it stays apart from own text before it
            if let Span::Disk(a, _) = span {
                if !anchored && start <= lo && lo < end {
                    left.push(Span::Disk(*a, *a));
                    anchored = true;
                }
            }
            if hi > end || (len == 0 && (lo == start || lo >= end)) {
                right.push(cut(span, end.clamp(lo, hi) - lo, len));
            }
        }
        left.push(Span::Own(text.into()));
        left.extend(right);

        // own text that puts back what it replaced is disk again
        let mut spans: Vec<Span> = vec![];
        for span in left {
            if let Span::Disk(c, d) = span {
                let owns = spans
                    .iter()
                    .rev()
                    .take_while(|s| matches!(s, Span::Own(_)))
                    .count();
                if let Some(Span::Disk(a, b)) = spans.iter().rev().nth(owns) {
                    let own: String = spans[spans.len() - owns..]
                        .iter()
                        .map(|s| match s {
                            Span::Own(text) => text.as_str(),
                            Span::Disk(..) => "",
                        })
                        .collect();
                    if owns > 0 && *b <= c && own == self.landed[*b..c] {
                        let a = *a;
                        spans.truncate(spans.len() - owns - 1);
                        spans.push(Span::Disk(a, d));
                        continue;
                    }
                }
            }
            spans.push(span);
        }
        self.spans = spans;
    }

    /// The local edits: each gap between disk runs that has own text or skips
    /// some of `landed`, as a byte range of `landed`.
    fn edits(&self) -> Vec<(usize, usize)> {
        let (mut edits, mut end, mut own) = (vec![], 0, false);
        for span in &self.spans {
            match span {
                Span::Disk(a, b) => {
                    if own || *a > end {
                        edits.push((end, *a));
                    }
                    (end, own) = (*b, false);
                }
                Span::Own(text) => own |= !text.is_empty(),
            }
        }
        if own || end < self.landed.len() {
            edits.push((end, self.landed.len()));
        }
        edits
    }

    /// Merges `disk` in and returns what the note should show.
    fn reload(&mut self, disk: &str) -> String {
        let segs = lb_rs::model::text::unicode_segs::calc(&self.landed);
        let remote: Vec<_> = lb_rs::model::text::diff(&self.landed, disk)
            .into_iter()
            .map(|r| {
                let (s, e) = segs.range_to_byte(r.range);
                (s.0, e.0, r.text)
            })
            .collect();
        let overlap =
            |(a0, a1): (usize, usize), (b0, b1): (usize, usize)| match (a0 == a1, b0 == b1) {
                (false, false) => a0 < b1 && b0 < a1,
                (true, false) => b0 < a0 && a0 < b1,
                (false, true) => a0 < b0 && b0 < a1,
                (true, true) => false,
            };
        let edits = self.edits();
        let dropped: Vec<_> = remote
            .iter()
            .map(|(s, e, _)| edits.iter().any(|edit| overlap(*edit, (*s, *e))))
            .collect();

        // what a dropped edit would have changed stays as the note's own text;
        // the note's ends stay runs of disk, if empty, to hold inserts there
        let len = self.landed.len();
        let mut spans = vec![];
        for span in std::mem::take(&mut self.spans) {
            let Span::Disk(a, b) = span else {
                spans.push(span);
                continue;
            };
            let mut at = a;
            for ((s, e, _), _) in remote.iter().zip(&dropped).filter(|(_, d)| **d) {
                let (s, e) = ((*s).max(at), (*e).min(b));
                if s < e {
                    if s > at || s == 0 {
                        spans.push(Span::Disk(at, s));
                    }
                    spans.push(Span::Own(self.landed[s..e].into()));
                    at = e;
                }
            }
            let inside = remote
                .iter()
                .zip(&dropped)
                .any(|((s, e, _), d)| *d && *s < a && a < *e);
            if at < b || (a == b && !inside) || b == len {
                spans.push(Span::Disk(at, b));
            }
        }

        // where a byte of `landed` lands in `disk`, past the inserts there if `past`
        // a byte inside a remote edit lands at its start
        let to_disk = |p: usize, past: bool| -> usize {
            let p = remote
                .iter()
                .find(|(s, e, _)| *s < p && p < *e)
                .map_or(p, |(s, ..)| *s);
            let mut q = p;
            for (s, e, text) in &remote {
                if *e < p || (*e == p && (s < e || past)) {
                    q = q + text.len() - (e - s);
                }
            }
            q
        };
        let mut next = vec![];
        for (i, span) in spans.iter().enumerate() {
            let Span::Disk(a, b) = span else {
                next.push(span.clone());
                continue;
            };
            // an insert at this run's end belongs to the next run if nothing of
            // `landed` lies between them, after any own text inserted there
            let next_start = spans[i + 1..].iter().find_map(|later| match later {
                Span::Disk(a, _) => Some(*a),
                Span::Own(_) => None,
            });
            let past = next_start != Some(*b);
            next.push(Span::Disk(to_disk(*a, false), to_disk(*b, past)));
        }
        let end = disk.len();
        if !matches!(next.last(), Some(Span::Disk(a, b)) if *a == end && *b == end) {
            next.push(Span::Disk(end, end));
        }
        self.landed = disk.into();
        self.spans = next;
        self.shown()
    }
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

/// The shortest case behind each row, against the buffer alone.
#[test]
fn reload_cases() {
    use lb_rs::model::text::buffer::Buffer;
    use lb_rs::model::text::operation_types::{Operation, Replace};

    // reading only: one new version
    let mut b = Buffer::from("foo 👋 foo\n");
    b.reload("foo 👋foo  foo\n".into());
    b.update();
    assert_eq!(b.current.text, "foo 👋foo  foo\n");

    // typing one letter between versions
    let mut b = Buffer::from("foo foo foo foo foo\n");
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
