# Blocks

Vertical units with chrome. You know the blocks.

The space *between* them is not a block. The parser skips empty lines, which is fine for a renderer and wrong for an editor: if the user adds a newline between two blocks, they must see it. Spacing exists because we are interactive, not because markdown has a “gap node.”

[Edits](edits.md): lists, quotes, and tasks are multi-line containers you pretend you aren’t in; headings and table cells are not; innermost first. [Reveal](reveal.md): hide markers; empty ATX headings still show `#` so the block doesn’t vanish. Fold details later; reorder later except as noted.

## Headings

ATX only, for the same WIP-editing reason setext is off ([dialect](dialect.md)). Would be nice to have setext back if we can nail the hyphen-under-paragraph case. Nobody seems to use them much.

Headings fold. That has been enough.

## Lists and tasks

Bullets, numbers, tasks. Tight vs loose: we don’t distinguish, and nobody knows or cares. Visualizing it would be the lowest-priority thing of all time. Open in theory.

**List items are reorderable** — users asked. Hiding the software keyboard while you reorder is a nice-to-have, not a requirement ([ime](ime.md)).

Lists fold, same as headings.

Task checkboxes are chrome you tap. **Not** in a read-only view ([identity](identity.md)).

**Gap:** a numbered task item is valid GFM (`1. [ ] …`) and is broken for us.

## Quotes and alerts

Quotes are the “pretend you aren’t in it” container. Alerts are GitHub-tagged quotes. Whatever styles are missing from the toolbar, we intend to add; quotes just need an entry.

Folding quotes (or other non-list/heading blocks) would be nice. Fold buttons clutter an otherwise clean page; with the right progressive disclosure, open to it.

## Code

Fenced blocks with a language. Syntax highlighting is not something we’re pleased with, but it works. Weak Syntect / Tree-sitter direction is [identity](identity.md). Which languages we skip because the highlighter panics is an engine scar, not a product list.

**Four-space indented code** is leftover CommonMark. We don’t want to break CommonMark. It still bites people: tab-indent a paragraph and it becomes an unlabeled code box (color + chrome, no language) instead of indenting. Confusing, not optional to support.

Inside a fence, it would be nice to **embrace code**: tab inserting spaces, a following line inheriting indent. We have a primitive “match the line above.” The rest is aspirational. [Edits](edits.md) already said code is a different, lower-priority feature set.

## Tables

Today: don’t explode. They are coming up **soon as first-class**. Users have gone to pencil and paper because ours are too weak. Target: cell-by-cell like a spreadsheet — Tab and Return navigate like Excel; chrome to add and rearrange rows and columns. Sorts, even charts, are cool territory, not a promise. Not on the toolbar yet because they are still a proof of concept.

## The rest

**Front matter:** often rendered table-like (Obsidian). That’s a good product direction. Today it is a code block ([dialect](dialect.md)).

**HTML:** syntax-highlighted source. GitHub’s finite vocabulary later, as already noted.

**Thematic break:** a thematic break. Be normal.

**Footnote definitions:** we tried; nobody used them; not worth maintaining. Can support later. Not a current feature.

## Toolbar

Intend to add the missing block styles on some timeline. Quotes need a button. Tables wait until they are first-class.
