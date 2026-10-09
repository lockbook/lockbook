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

## Tables
Target: first-class, cell-by-cell like a spreadsheet — Tab and Return navigate like Excel; chrome to add and rearrange rows and columns. Sorts and charts are cool territory, not a promise. They wait on the toolbar until they are first-class.

## Other
**Front matter:** Obsidian-like table rendering is the product direction; a code-block stand-in is fine until then ([dialect](dialect.md)).

**HTML:** syntax-highlighted source, with GitHub's finite vocabulary in the target state.

**Thematic break:** a thematic break.
