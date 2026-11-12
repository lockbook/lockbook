//! Links, as the editor writes and opens them.

use lb_rs::Uuid;
use lb_rs::model::text::offset_types::Grapheme;

use super::super::input::{Event, Location, Region};
use super::harness::TestEditor;
use crate::file_cache::FileCache;
use crate::test_utils::files::{at, cache, tree};

/// `/budget.md`, and a `/project/` shared with bob.
fn shared_project() -> FileCache {
    cache(tree(
        "alice",
        &[
            "/budget.md",
            "/project/ @bob",
            "/project/notes.md",
            "/project/plan.md",
            "/project/design/plan.md",
            "/project/design/sketch.svg",
            "/project/chart.png",
        ],
    ))
}

/// Paste `pasted` into `md` in `/project/notes.md` with `selection` selected.
fn paste(md: &str, selection: (usize, usize), pasted: &str) -> String {
    let files = shared_project();
    let note = at(&files, "/project/notes.md");
    let mut ws = TestEditor::in_files(files, note, md);
    ws.push(Event::Select {
        region: Region::BetweenLocations {
            start: Location::Grapheme(Grapheme(selection.0)),
            end: Location::Grapheme(Grapheme(selection.1)),
        },
    });
    ws.enter_frame();
    let event = ws.editor.edit.paste_event(pasted.into()).unwrap();
    ws.push(event);
    ws.enter_frame();
    ws.get_text().to_string()
}

#[test]
fn pasted_id_links_are_written_for_the_note() {
    let files = shared_project();
    let id = |path: &str| at(&files, path);
    let sketch = format!("https://app.lockbook.net/open/{}", id("/project/design/sketch.svg"));
    let plan = format!("lb://{}#next-steps", id("/project/design/plan.md"));
    let budget = id("/budget.md");

    // in the scope: a relative path, labeled with the file's name
    assert_eq!(paste("see ", (4, 4), &sketch), "see [sketch](design/sketch.svg)");
    assert_eq!(paste("see ", (4, 4), &plan), "see [plan](design/plan.md#next-steps)");
    // out of it: the external URL
    assert_eq!(
        paste("see ", (4, 4), &format!("lb://{budget}")),
        format!("see [budget](https://app.lockbook.net/open/{budget})")
    );
    // over a selection, the selection is the label
    assert_eq!(paste("see this", (4, 8), &sketch), "see [this](design/sketch.svg)");
    // a link begun by hand is the writer's own: the URL pastes as it is
    assert_eq!(paste("see [[", (6, 6), &plan), format!("see [[{plan}"));
    assert_eq!(paste("see [it](", (9, 9), &sketch), format!("see [it]({sketch}"));
    assert_eq!(paste("see [it](x)", (7, 7), &sketch), format!("see [it{sketch}](x)"));
    assert_eq!(paste("see `at `", (8, 8), &sketch), format!("see `at {sketch}`"));
    // a link to this note is a bare name; to one of its headings, just the
    // heading, by its text
    let notes = format!("lb://{}", id("/project/notes.md"));
    assert_eq!(paste("see ", (4, 4), &notes), "see [notes](notes.md)");
    assert_eq!(
        paste("# Next steps\n\nsee ", (18, 18), &format!("{notes}#next-steps")),
        "# Next steps\n\nsee [Next steps](#next-steps)"
    );
    // links to anything else paste as they are
    let unknown = format!("lb://{}", Uuid::from_u128(99));
    assert_eq!(paste("see ", (4, 4), &unknown), format!("see {unknown}"));
    assert_eq!(paste("see ", (4, 4), "https://lockbook.net"), "see https://lockbook.net");
}

#[test]
fn pasted_url_links_a_selection_ending_at_a_link() {
    let pasted = paste("hello\n\n[x](y)", (2, 7), "https://lockbook.net");
    assert!(pasted.contains("](https://lockbook.net)"), "{pasted:?}");
}

/// Whether completions are open after typing `typed` at `caret` in `md`,
/// one grapheme a frame, in `/project/notes.md`.
fn completes(md: &str, caret: usize, typed: &str) -> bool {
    let files = shared_project();
    let note = at(&files, "/project/notes.md");
    let mut ws = TestEditor::in_files(files, note, md);
    ws.push(Event::Select { region: (Grapheme(caret), Grapheme(caret)).into() });
    ws.enter_frame();
    for c in typed.chars() {
        ws.push(Event::Replace {
            region: Region::Selection,
            text: c.to_string(),
            advance_cursor: true,
        });
        ws.enter_frame();
    }
    // the popup reads the buffer as a frame starts, and a parked caret
    // must keep it closed past its first frame
    ws.enter_frame();
    ws.enter_frame();
    ws.editor.edit.link_completions.active
}

/// A link typed between words completes as one typed at the end of a line,
/// even before another link.
#[test]
fn link_completions_open_mid_line() {
    let mut closed = vec![];
    for typed in ["[[pl", "![[ch", "[pl", "![ch"] {
        for md in ["see ", "see  after", "see  and [[other]]", "see  and [x](y)"] {
            if !completes(md, 4, typed) {
                closed.push(format!("{typed:?} in {md:?}"));
            }
        }
    }
    assert!(closed.is_empty(), "closed: {closed:#?}");
}

/// Typing in a closed wikilink completes as in an unclosed one, `#` its
/// headings, but moving the caret into one opens nothing.
#[test]
fn closed_wikilinks_complete_as_typed() {
    assert!(completes("# Next steps\n\nsee [[]]", 20, "#"));
    assert!(completes("see [[pl]]", 8, "a"));
    for md in ["see [[Plan]]", "see [[plan.md]]", "see [[pla]]", "see [[]]", "see ![[chart.png]]"] {
        assert!(!completes(md, md.find("]]").unwrap(), ""), "{md}");
    }
}

/// A link to a file is a pill unless it was given a title of its own.
#[test]
fn file_links_are_pills_unless_titled() {
    let is_pill = |md: &str| {
        let files = shared_project();
        let note = at(&files, "/project/notes.md");
        let plan = at(&files, "/project/plan.md");
        let ws = TestEditor::in_files(files, note, &md.replace("{plan}", &plan.to_string()));
        let fragments = &ws.editor.edit.renderer.fragments;
        fragments
            .iter()
            .any(|f| f.style_stack.last().is_some_and(|s| s.chip))
    };
    assert!(is_pill("See [[plan]] here."));
    assert!(is_pill("See <lb://{plan}> here."));
    assert!(!is_pill("See [[plan|the plan]] here."));
    assert!(!is_pill("See [the plan](lb://{plan}) here."));
}

/// A read-only view has no cursor to place, so a click on a pill opens it.
#[test]
fn read_only_pill_opens_on_click() {
    use crate::tab::ExtendedOutput as _;
    let files = shared_project();
    let note = at(&files, "/project/notes.md");
    let plan = at(&files, "/project/plan.md");
    let mut ws = TestEditor::in_files(files, note, "See [[plan]] here.");
    ws.editor.edit.renderer.readonly = true;
    ws.enter_frame();
    let renderer = &ws.editor.edit.renderer;
    let pill = renderer
        .fragments
        .iter()
        .filter(|f| f.style_stack.last().is_some_and(|s| s.chip));
    let at = pill
        .map(|f| f.rect)
        .reduce(egui::Rect::union)
        .unwrap()
        .center();
    let button = |pressed| egui::Event::PointerButton {
        pos: at,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    ws.enter_frame_with_input(vec![egui::Event::PointerMoved(at)]);
    ws.enter_frame_with_input(vec![button(true)]);
    ws.enter_frame_with_input(vec![button(false)]);
    let opened = ws.editor.edit.renderer.ctx.pop_open_files();
    assert_eq!(opened.first().map(|o| o.0), Some(plan));
}

/// #5109
#[test]
fn toggle_link_twice_restores_text() {
    let link = || Event::ToggleStyle {
        region: Region::Selection,
        style: comrak::nodes::NodeValue::Link(Default::default()),
    };
    for (md, (start, end)) in [("hello", (0, 0)), ("hello world", (0, 5))] {
        let mut ws = TestEditor::new(md);
        ws.enter_frame();
        ws.push(Event::Select { region: (Grapheme(start), Grapheme(end)).into() });
        ws.enter_frame();
        ws.push(link());
        ws.enter_frame();
        ws.push(link());
        ws.enter_frame();
        assert_eq!(ws.get_text(), md);
    }
}
