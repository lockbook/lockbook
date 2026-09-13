# Dialect

We speak **GitHub Flavored Markdown plus some extensions**. It will not perfectly match any other client. We add things people wanted that are not in GFM — highlights are the example (`==…==`); they are not GFM, they are a parser extension, and Bear puts them first on their marketing site.

We look most closely at **GitHub** and **Obsidian** as compatibility targets. Implement something that already works in at least one other client; do not go off on our own. Copy-paste with Bear for folds is the kind of interop we want wherever we can afford a good experience. Paste from Obsidian should not break if we can help it; we can break it if we have to.

In the target state, **everything we parse is so we can deliver a useful feature.** We do not parse “just in case.”

## One dialect, not settings

The document must show up the same way for every user. No per-user choice of syntax. We pick what is correct, document it, and impose it so people stay compatible with each other and with the tools we chose. Underline is `__` (what we insert today). A settings-negotiated dialect would be a worse story.

The construct list is still tunable while the app is in development. In a near-to-medium-term resting state it will not be — perhaps one or two additions a year.

## Soft breaks are hard

The spec leaves soft-break rendering to the implementer. We render **soft breaks as hard breaks, everywhere** — the note, the composer, previews. That is what an interactive editor needs.

Obsidian does this in the editor and then has a separate rendered view that does not, plus a lot of documentation explaining the difference. We skip that complexity. If we ever export to something that will not respect this (a static site generator, GitHub’s HTML preview), that is a compatibility note or a runtime check at the export boundary — not a second rendering mode in the app.

## Interactive editing vs a batch parser

Some CommonMark behavior is faithful and still wrong *while you type*. Setext headings are off because starting a hyphen-bullet under a paragraph briefly turned that paragraph into a heading; users found it very confusing. A lone hyphen on the next line is a one-item empty list — unless there is no blank line after the paragraph, in which case it is just a hyphen. Confusing, spec-correct, and we live with what the parser reports.

We have needs a traditional markdown parser does not: work-in-progress syntax should not surprise people mid-keystroke.

## Parser (not a dialect)

Comrak source positions are not trusted. It allocates heavily and is not set up to retain a parse across frames, so we re-parse and copy more than we need every frame — we mostly want source positions so we can slice *our* document, not copies of its text.

We are open to our own parser, motivated by performance, correctness, small tweaks to work-in-progress syntax, and **disjoint ranges** (a fenced code block inside a quote is interleaved with `>` prefixes — that is what blocks sharing a code-block widget with a code-file widget, and a table with a CSV). See [links-images](links-images.md). That is not a license to invent a Lockbook-only language.

## Extensions and leftovers

**Highlights, spoilers, tasks, tables, alerts, underline, sub/superscript, strikethrough, autolink, wiki links.** Features we mean. Wiki links follow Obsidian’s title-after-pipe. Obsidian-shaped extras (including image width/height) are part compatibility, part “that feature is how you do this in markdown.” GitHub’s alternative for size is an HTML `<img>`, which is not markdown; we are a markdown editor, not an HTML editor.

**Math** (`$…$` and code-span math): we want to draw it as math. Not there yet; parsed so we can get there.

**Front matter** (`---`): we like it; Obsidian’s support is the reference; users have asked. Today it shows as a code block, which is fine and should get better. A todo, not a non-feature.

**Emoji shortcodes** (`:smile:`): we like them and like that they interop. Considering inserting the emoji character from completions so shortcodes mostly leave the file in the common case. Still parse them.

**Footnotes:** nobody has asked. Wouldn’t mind. Not a priority.

**Description lists, greentext, multiline block quotes:** intended off. They create compatibility or editing annoyances.

**Smart punctuation:** on in the parser, irrelevant in practice — we take glyphs from our own source text, not from the parser’s rewritten copy.

**Setext headings:** off today because of that WIP hyphen-under-paragraph case. Nobody seems to use them much; we would take them back if we could nail that interactive editing moment. The public syntax guide still documents them.

**HTML:** the fold tag is an HTML comment in the file ([document-model](document-model.md)). We are open to rendering the finite HTML vocabulary GitHub supports, for compatibility — not because anyone wants to type it. We do not want to grow HTML comments as a general pattern (same flaws as folds). Avoid if we can.

## Not this note

- How each construct looks and edits — blocks, inlines, folding.
- A decaying checklist of comrak flags. The promise is GFM + the extras above, one dialect, GitHub and Obsidian as the people we track.
