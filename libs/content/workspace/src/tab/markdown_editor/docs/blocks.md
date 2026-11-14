# Blocks
Headings, lists, quotes, code, tables, and the rest. Lists, quotes, and tasks are container blocks you can keep typing in as if you weren't in them ([edits](edits.md)). Empty headings keep their `#`s so the block still has a presence ([reveal](reveal.md)).

## Headings
ATX only — setext is off because a hyphen-bullet under a paragraph briefly became a heading ([dialect](dialect.md)). We would take setext back if that interactive moment could be nailed. Headings fold ([folding](folding.md)).

## Lists and tasks
Bullets, numbers, and tasks. Tight vs loose is undistinguished.

List items are [reorderable](reorder.md) and [fold](folding.md). Task checkboxes are chrome you tap; they are an edit, so they are off in a read-only view ([identity](identity.md)). A numbered task item is valid GFM (`1. [ ] …`) and should work.

## Quotes and alerts
Quotes are the container-block reference case. Alerts are GitHub-tagged quotes. Quotes need a toolbar entry. Folding quotes (or other non-list/heading blocks) could be nice once disclosure doesn't clutter the page ([folding](folding.md)).

## Code
Fenced blocks with a language. Syntax highlighting uses TreeSitter in the target state. Four-space indented code is CommonMark we still support; tab-indenting a paragraph becoming an unlabeled code box is confusing but happens.

Inside a fence, code should behave more like a code editor on a best-effort basis: tab inserting spaces, a following line inheriting indent ([edits](edits.md)).

## Diagrams
A closed ```` ```mermaid ```` fence draws as its diagram, the format GitHub, GitLab, and Obsidian render, so notes stay portable. Collapsed, the fence is one atom like an image ([rich-inlines](rich-inlines.md)): a tap selects it, Edit Diagram puts the caret in its source, a cmd-click opens it in a tab, and a backspace selects it before deleting it. With the caret inside, the source shows as code with the last render that worked below it, so the picture holds while you type; an error shows above that picture, and the source is underlined where the error points, as a misspelling is. An unclosed fence stays code, since it runs to the end of the note.

Diagrams render off the UI thread, measured and drawn with the app's own fonts and colored from the theme. Copy Image puts the picture on the clipboard, and Copy Rendered pastes it as an image.

## Tables
A table is a grid, cell-by-cell like a spreadsheet. The caret lives in cells and steps from one to the next, never onto a pipe, a cell's padding, or the delimiter row; Tab and Return navigate like Excel and extend the table at its end. A selection takes whole cells or whole rows once it leaves a cell ([selection](selection.md)), and no edit joins two cells or leaves a row that is not a row.

Rows and columns are inserted, deleted, moved, and aligned from the toolbar or from handles on the table itself ([chrome](chrome.md), [reorder](reorder.md)). Each is one undo unit on the rows' source lines that leaves every other cell as it was written. Columns are as wide as their content; cells wrap before the table outgrows the note, and past that the table scrolls ([layout](layout.md)).

The delimiter row has no place in the grid. A table shows as source only while the caret is on it: someone is typing a table by hand. Sorts and charts are cool territory, not a promise.

## Other
**Front matter:** a grid at the top of the note, a row per property with the key in one column and the value in the other; the fences, colons, dashes, and quotes are hidden and are atoms for the caret. A list is a row of chips, `tags` in the accent; a plain `true` or `false` is a checkbox. Return starts a row or the next item, a comma ends an item, Tab moves key to value to the next row, a colon ends a key being typed, and a value that comes to need quotes gets them as it is typed. The toolbar's property button and ⌘; add a property; the row menu moves, removes, and changes a value's kind. Every edit replaces one key, value, item, or line, so comments, order, quoting, and list style survive untouched. YAML the grid does not edit in place — comments, nested maps, multi-line scalars, and lines that fail to parse — shows as source rows in the same grid, so one bad line never costs the block ([dialect](dialect.md)).

**HTML:** syntax-highlighted source, with GitHub's finite vocabulary in the target state.

**Thematic break:** a thematic break.
