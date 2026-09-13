# Markdown editor — working notes

Pairing notes for a requirements catalog of the editor in this directory. Not the user-facing syntax guide (`docs/editor.md` at the repo root).

One file per area. Code is evidence of what exists; these notes are the source of truth for what must hold.

Prose is one sentence per line, or a paragraph as a single line — no hard wrap. The viewer wraps.

## Areas

- [x] [identity](identity.md) — a text stack of building blocks, not a flagged monolith
- [x] [reveal](reveal.md) — hide syntax so edit is preview; show it so the caret has a place and so the toolbar can teach markdown
- [x] [document-model](document-model.md) — the bytes on disk; many writers; undo as interpretation; graphemes at this layer are a known flaw
- [x] [dialect](dialect.md) — GFM plus extensions; GitHub and Obsidian; one dialect, not settings
- [x] [layout](layout.md) — wrap to the column; huge notes; stay put; scroll is a building block
- [x] [selection](selection.md) — one ordered range, or none; native on each platform; rich-text around images
- [x] [ime](ime.md) — one keyboard contract; native typing; solid for current users before CJK
- [x] [edits](edits.md) — Docs-ish: source replacements of a parsed structure; innermost first
- [x] [blocks](blocks.md) — vertical units with chrome; tables first-class soon; indented code is CommonMark leftover
- [x] [inlines](inlines.md) — circumfix family; two leaves (text, inline code); Return always shows
- [x] [rich inlines](rich-inlines.md) — images, chips, cards, fold `···`; stand-ins for source
- [x] [folding](folding.md) — hide finished writing; headings and lists; reader-local someday; mobile long-press is a good idea
- [x] [reorder](reorder.md) — sibling list items; marker on desktop, long-press on phone; no Alt-Down
- [x] [links-images](links-images.md) — file or web; wiki click-to-create; fetch default off; attachments as a share unit; embeds beyond images
- [x] [chrome](chrome.md) — hideable toolbar; find as a POC; completions ignorable; menus native-bound
- [x] [appearance-perf](appearance-perf.md) — app palette; OS text size; no wasted work; system fonts for CJK
