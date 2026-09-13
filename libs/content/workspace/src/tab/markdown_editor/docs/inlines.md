# Inlines

Stuff inside a paragraph. Circumfix styles are a consistent family; that is working for us. Images, cards, and wiki *resolution* are later notes. A link as clickable text is here; the pretty chip/block is later.

## Circumfix family

Emphasis, strong, strike, highlight (`==`), underline (`__`), sub, super: hide the markers, look like rich text, nest. Toolbar / Cmd+B applies or unwraps ([edits](edits.md), [reveal](reveal.md) as teacher).

**Two leaves:** plain text, and **inline code**. Inline code does not nest (no bold inside a code span). Combinations of the others are allowed. We may be missing some font variants (italic + mono + bold); those combos are best-effort. Users are pleased. The missing faces are an engine gap, not a “don’t nest” rule.

**Asked for, not shipped:** copy the *contents* of an inline code span (a user with a bulleted list of commands to paste into a terminal).

Empty / WIP circumfix while you type: be normal, same spirit as other WIP syntax ([dialect](dialect.md)).

## Spoilers

A different kind of reveal than hiding `* *` ([reveal](reveal.md)). Today they are half-finished: maybe hover, maybe tap-to-toggle, and they may reset when the document is edited. **Aim for what’s normal** (tap/click to show, caret inside to edit). Don’t freeze the current half-state.

## Math

Parsed so we can get there. Success is **inline math**, in the line, something like an inline image, maybe aligned a little differently. Until then, code-styled `$…$` is the stand-in.

## Shortcodes and emoji

Parse `:smile:`. Completions may insert the emoji character so shortcodes mostly leave the file. Still parse them so typing `:smile:` works and paste from elsewhere doesn’t break.

## Breaks

If the user hits Return, a newline goes in the file and **they see it**. We do not hide newlines. In no situation does Return leave the document looking unmodified.

[Dialect](dialect.md): source newlines render as hard breaks everywhere, including previews. CommonMark also has an invisible hard-break (two trailing spaces before the newline, or a backslash). That distinction barely shows up for us because we already show every source newline. Not a product we are nursing.

## Links (as text)

We have thrashed. It still depends on the job:

- **Read-only / show:** click **opens**. Cmd-click could be a new tab; for a web URL it barely matters.
- **Editing:** tap currently brings a Slack-like “edit the link” (caret on the URL). More chrome would help — title vs destination is easy to mix up.

Chip vs block vs mobile “Edit” — [rich inlines](rich-inlines.md).

Autolink (`https://…`) is a feature we mean ([dialect](dialect.md)).

## Escapes and HTML

`\*` is a star. Be normal. No special Lockbook story.

Inline HTML: **show source**, unless it is a type we explicitly support. Same as HTML blocks. GitHub and Obsidian support a little inline HTML; we may take that finite vocabulary later. Not an HTML editor.

## Jobs

**Note and composer:** all of this carries over.

**Show / read-only:** rendered styles, tappable spoilers (what’s normal), links open. No apply-style.

**Plaintext / code:** not this family.

## Not this note

- Images, width/height, cards, wiki file resolution, drop/camera.
- Footnote references: same as definitions — unused, can support later ([blocks](blocks.md)).
- Which font files we bundle.
