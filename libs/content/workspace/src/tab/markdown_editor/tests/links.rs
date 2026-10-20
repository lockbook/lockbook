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
    // a link to this note is a bare name
    let notes = format!("lb://{}", id("/project/notes.md"));
    assert_eq!(paste("see ", (4, 4), &notes), "see [notes](notes.md)");
    // links to anything else paste as they are
    let unknown = format!("lb://{}", Uuid::from_u128(99));
    assert_eq!(paste("see ", (4, 4), &unknown), format!("see {unknown}"));
    assert_eq!(paste("see ", (4, 4), "https://lockbook.net"), "see https://lockbook.net");
}
