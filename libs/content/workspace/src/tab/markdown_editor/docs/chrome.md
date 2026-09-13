# Chrome

The stuff around the document: toolbar, find, completions, menus. Applying a style *is* [edits](edits.md); showing the stars so it teaches is [reveal](reveal.md). Completions as a *way in* for links is [links-images](links-images.md). This note is the chrome jobs.

**Note:** all of this. **Composer:** completions in; find out; toolbar not required here but cheap to add ([identity](identity.md)). **Show / single-line:** none of this.

## Toolbar

Users want an **option to hide it**.

**Customize which buttons** — **mobile only**, an experiment. Not super intuitive. It earns its keep two ways: it **teaches** what a button means (without having to try it on the document), and with so many markdown elements it creates **focus** so people don’t scroll a mile of icons like some other apps.

The **heading** button **cycles levels** — every heading level in one control.

Quotes still need an entry; tables wait until they’re first-class ([blocks](blocks.md)). Which buttons exist is allowed to grow; don’t freeze today’s set.

Toolbar *layout* persistence is not the document ([document-model](document-model.md)).

## Find and replace

Code-editor-style find/replace is nice: **navigate matches**, **replace one**, **replace all**. **Find in selection** does not feel useful.

Implementation is a bit buggy, **especially on iOS** around native text fields. Treat it as a **workable proof of concept**, not a finished native find.

Find does **not** steal the caret until you decide ([reveal](reveal.md) peek for folds). Composer: out.

On iOS a **native search button** that opens this is kind of nice.

## Completions

Emoji on `:`. Files/wiki/images on `[` / `[[` / `![` — they pick the syntax by how they start ([links-images](links-images.md)). May insert the emoji **character** instead of the shortcode ([dialect](dialect.md)).

Active work is making the windows **nice, placed well, contents fitting**.

You should be able to **ignore** them while typing with **no side effect**, and also **submit easily**. Those conflict when Return submits instead of inserting a newline. **Acceptable.** Max results is details.

Completions need **not** be native widgets.

Especially good in the composer.

## Menus

Visual update coming.

**iOS / Android:** ideally **native** system menus (same bar as [ime](ime.md) / [selection](selection.md)).

**macOS:** ideally native eventually. For now the Lockbook desktop design system — already inspired by macOS context menus — is a workable placeholder until we do that work.

Link Open / Edit / Copy: lightweight, not decided ([rich inlines](rich-inlines.md)). Copy-as-plain / copy-as-markdown: asked for ([edits](edits.md)); a natural home if we add them.

## Not this note

- Button padding, popup pixel math, `ToolbarPersistence` field list.
- Regex find — not discussed.
