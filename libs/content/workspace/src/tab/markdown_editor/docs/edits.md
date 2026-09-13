# Edits

The overall feel is **Google Docs-ish**: markdown pretending to be rich text. It should be nice to backspace a bullet away, or to have the next item appear at the right moment. We lean into that.

The algorithms are dense because the product is defined on the **parsed structure**, and every change still has to be a **source replacement** ([document-model](document-model.md): the file is the bytes). That translation is delicate. Fragility here is expected; it is not a license to skip the rich-text feel.

[Reveal](reveal.md) already has the toolbar as a markdown teacher. [Selection](selection.md) already has backspace-beside-an-image deleting the whole glyph. Compound actions undo as one gulp ([document-model](document-model.md)).

## Pretend you are not in the container

For **multi-line blocks** — lists, quotes, tasks, and the like — you should be able to pretend you are *not* in that container and just keep typing. What you write is exactly what you would have written, except it is in the quote (or list, or …). Return, for example, puts the quote prefix on the next line for you.

This does **not** include headings or table cells. Code blocks are excluded from this story for now (they have their own indent habits; see below).

Nested GFM: the same features are available inside a list-in-a-quote as outside. When there is ambiguity, **innermost first**, so it feels nested. Minor edge-case exceptions are allowed.

## Return

Return **inserts the next thing** — a new list item, another quoted line, and so on.

Shift+Return **softens** that: extend the current thing, adding only the indentation that continuation needs. Cmd+Return could exist if we needed a third degree; it has not been necessary.

## Backspace and ranges

With a **single caret**, backspace generally peels **one nesting layer** — a whole prefix at a time (the bullet, the `>`, one level).

With a **range**, delete that range, whatever it is. The smart behaviors are mostly for a caret, not a selection.

## Tab and NBSP

Tab **indents / nests** (and Shift+Tab outdents). It does **not** insert a tab character today.

That is a workaround, not a virtue. Tab characters made the editor unstable — GitHub treats a tab as a variable number of spaces depending on column, and we had a stream of visually confusing bugs — so we disabled them. It is embarrassing that a text editor cannot have tabs. We would like to support them. Until then, do not treat “no `\t`” as the product.

**Non-breaking spaces** are the same category. Paste currently replaces U+00A0 with a normal space (`parser does not interact well with non-breaking spaces` — they often arrive from AI output). We would like to keep them. Flattening on paste is not the product.

## Copy and paste

**Today (best-effort):** the clipboard is the **source characters** you selected. Empty selection: the whole source line (macOS-ish). Current behavior is a little funny (caret can land on the wrong line after). The target is that model, not the bug.

**Asked for, not shipped:**

- Copy as **plain text** — drop the styling punctuation.
- Copy as **markdown** — even if the selection is only the inside of a bold, put the opening `**` on the clipboard too. That would also help a [reveal](reveal.md) / [layout](layout.md) bind: you cannot select hidden characters (heading `#`s) during a drag, only after release when they appear. A markdown copy would not need you to hunt them.
- Defaults and/or context-menu choices among those.
- OS **multi-format** clipboard, so wherever you paste we make a best effort to keep formatting. Open to it.

## Apply style

Cmd+B / toolbar Bold (and the rest) wrap or unwrap. The implementation has to decide apply vs unapply, then splice and extend regions. Tricky, a little fragile, solid enough after a few passes.

**Refusal** would be good and is not really there: if you are in a code block (or anywhere inlines cannot exist), don’t pretend to bold.

Applying a style should still *show* the syntax in that moment so it can teach ([reveal](reveal.md)).

## Jobs

**Note and composer:** this smart typing.

**Plaintext files** (not markdown): mixed. Some people want to edit *markdown* as a monospace source view with highlighting **and** some of these smart features — GitHub.com’s list auto-insert in an otherwise plain editor. One user (a friend) has asked; nobody else has; we have put it off and still want it on some timeline. That is not “turn ToggleStyle off in `.txt`.” It is a future job, closer to GitHub’s editor than to today’s non-`.md` path.

**Code:** a **different** feature set, not this one with the lights off. Indentation is more important and more rigid; tab characters and tab-as-indent belong there. Lower priority than markdown.

**Show / read-only:** no typing.

## Not this note

- Per-construct tables of every list/quote edge — those stay cheap to change; innermost-first is the rule.
- Drop, camera, paste-image — links-images.
- The `Event` enum. Newline / Delete / Indent exist because these are not dumb replaces; that is implementation.
