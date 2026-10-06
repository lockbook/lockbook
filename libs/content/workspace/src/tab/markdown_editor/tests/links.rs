//! Links, as the editor writes them.

use lb_rs::Uuid;
use lb_rs::model::text::offset_types::Grapheme;

use super::super::input::{Event, Location, Region};
use super::harness::TestEditor;
use crate::file_cache::{FileCache, FilesExt as _};
use crate::test_utils::files::{cache, tree};

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
        ],
    ))
}

/// Paste `pasted` into `md` in `/project/notes.md` with `selection` selected.
fn paste(md: &str, selection: (usize, usize), pasted: &str) -> String {
    let files = shared_project();
    let note = files.by_path("/project/notes.md").unwrap().id;
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
    let id = |path: &str| files.by_path(path).unwrap().id;
    let sketch = format!("https://app.lockbook.net/open/{}", id("/project/design/sketch.svg"));
    let plan = format!("lb://{}#next-steps", id("/project/design/plan.md"));
    let budget = id("/budget.md");

    // in the scope: a relative path, labeled with the file's name
    assert_eq!(paste("see ", (4, 4), &sketch), "see [sketch](design/sketch.svg)");
    assert_eq!(paste("see ", (4, 4), &plan), "see [plan](design/plan.md#next-steps)");
    // out of it: an id link
    assert_eq!(
        paste("see ", (4, 4), &format!("https://app.lockbook.net/open/{budget}")),
        format!("see [budget](lb://{budget})")
    );
    // over a selection, the selection is the label
    assert_eq!(paste("see this", (4, 8), &sketch), "see [this](design/sketch.svg)");
    // in wiki brackets, a title; in a destination, a destination
    assert_eq!(paste("see [[", (6, 6), &plan), "see [[design/plan#next-steps");
    assert_eq!(paste("see [it]()", (9, 9), &sketch), "see [it](design/sketch.svg)");
    assert_eq!(paste("see [it](", (9, 9), &sketch), "see [it](design/sketch.svg");
    assert_eq!(paste("see [[old]]", (6, 9), &plan), "see [[design/plan#next-steps]]");
    assert_eq!(paste("see `at `", (8, 8), &sketch), format!("see `at {sketch}`"));
    // links to anything else paste as they are
    let unknown = format!("lb://{}", Uuid::from_u128(99));
    assert_eq!(paste("see ", (4, 4), &unknown), format!("see {unknown}"));
    assert_eq!(paste("see ", (4, 4), "https://lockbook.net"), "see https://lockbook.net");
}
