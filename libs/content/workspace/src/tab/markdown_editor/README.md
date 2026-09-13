# Markdown editor
Design details of the markdown editor to support engineering.

Fundamentals: 
- [identity](docs/identity.md) — one source-preserving WYSIWYG; a text stack of building blocks, composed per job
- [document-model](docs/document-model.md) — the document is the bytes on disk; many writers; undo as interpretation
- [dialect](docs/dialect.md) — GitHub Flavored Markdown plus extensions; one dialect; GitHub and Obsidian
- [reveal](docs/reveal.md) — hide syntax so edit is preview; show it so the caret has a place and so the toolbar can teach
- [layout](docs/layout.md) — wrap to the column; huge notes; stay put; scroll is a building block
- [selection](docs/selection.md) — one ordered range, or none; native on each platform
- [ime](docs/ime.md) — one keyboard contract; the system draws caret, loupe, handles, and menu

Content: 
- [edits](docs/edits.md) — markdown pretending to be rich text; source replacements of a parsed structure
- [blocks](docs/blocks.md) — vertical units with chrome; spacing the parser would skip
- [inlines](docs/inlines.md) — circumfix styles; text and inline code; Return always shows
- [rich-inlines](docs/rich-inlines.md) — images, chips, cards, fold `···`; stand-ins for source
- [folding](docs/folding.md) — hide writing that is done; headings and lists
- [reorder](docs/reorder.md) — drag a list item among its siblings
- [links-images](docs/links-images.md) — file or web; wiki create; fetch privacy; attachments; embeds

Supporting features and polish: 
- [chrome](docs/chrome.md) — toolbar, find, completions, menus
- [appearance-perf](docs/appearance-perf.md) — app palette; OS text size; no wasted work; system fonts for CJK
