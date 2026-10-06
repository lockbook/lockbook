//! Links to files, as the editor writes them.

use lb_rs::Uuid;
use lb_rs::model::file::{File, Share, ShareMode};
use lb_rs::model::file_metadata::FileType;
use lb_rs::model::text::offset_types::Grapheme;

use super::super::input::{Event, Location, Region};
use super::harness::TestEditor;
use crate::file_cache::{FileCache, FilesExt as _};

fn file(id: u128, parent: u128, name: &str, file_type: FileType) -> File {
    File {
        id: Uuid::from_u128(id),
        parent: Uuid::from_u128(parent),
        name: name.into(),
        file_type,
        last_modified: 0,
        last_modified_by: String::new(),
        owner: "alice".into(),
        shares: vec![],
        size_bytes: 0,
    }
}

/// `/budget.md`, and a shared `/project/` holding `notes.md`, `plan.md`,
/// `design/plan.md`, and `design/sketch.svg`.
fn shared_project() -> FileCache {
    use FileType::{Document, Folder};
    let mut files = vec![
        file(1, 1, "alice", Folder),
        file(2, 1, "budget.md", Document),
        file(3, 1, "project", Folder),
        file(4, 3, "notes.md", Document),
        file(5, 3, "plan.md", Document),
        file(6, 3, "design", Folder),
        file(7, 6, "plan.md", Document),
        file(8, 6, "sketch.svg", Document),
    ];
    files[2].shares.push(Share {
        mode: ShareMode::Write,
        shared_by: "alice".into(),
        shared_with: "bob".into(),
    });
    FileCache::from_owned_and_shared(files[0].clone(), files, vec![])
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
