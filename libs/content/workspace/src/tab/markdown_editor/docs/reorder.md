# Reorder
Drag a list item (and everything inside it) to another place in the same list at the same indent level. A table's body rows reorder the same way among themselves, and its columns along the row; the header row stays the header. Indent, outdent, or drop into another list would be more fully featured and a lot more implementation and UX complexity — a step in perhaps the wrong direction.

It belongs in the note and the composer ([identity](identity.md)). This is an edit, so it is off in a read-only view.

## How you grab
**Desktop:** drag the marker. Hover changes the cursor to a hand — that is the only way to discover the feature. For task items this feels a bit strange and is perhaps undesirable. A table's row or column is dragged by the handle that shows on the table's border under the pointer.

**Phone:** long-press, then drag, so a pan still scrolls. We tried other gestures and didn't like what some popular apps do. In a table, a long-press on a body row drags the row and one on a header cell drags its column.

**Apple + hardware keyboard:** a finger-drag still scrolls ([ime](ime.md)). Reorder stays marker / long-press.

## After the drop
The only lasting impact is the thing you moved. Selection stays more or less as it was. Caret and scroll stay put except while dragging: auto-scroll near the viewport edges. On release, don't jump to the caret.

Numbered lists: the number we draw comes from position, so rewriting `1. 2. 3.` in the source often doesn't matter. Undo is one unit ([document-model](document-model.md)). A floating card is pleasant; whatever reads as "I'm dragging this item" is valid.
