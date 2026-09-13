# Selection

We are trying to follow what’s normal. The gestures, keys, and chrome below are not a Lockbook-specific spec — they are whatever it takes for the editor to feel like a standard text field on that platform. Freeze the *feel*, not a decaying inventory of clicks.

## What it is

The selection is an **ordered range**. One end is the caret (the end that moves with the arrows). The other is the far end — where a drag started, or simply the end that does not move.

There is **at most one** selection today. We are open to several (we are not a code editor, but multiple ranges would be nice). We also want **zero**. A range is not mandatory.

Zero matters because selection drives [reveal](reveal.md). On iOS a newly opened note sits at the default `(0, 0)`. Notes often start with a heading, so the first `#`s appear — but the system caret is not shown until the field is first responder, so you have not tapped yet and still get syntax. The right model for “not editing yet” is **no selection**, not a hidden caret at the start.

When there *is* a selection, **always show it.**

## Standard fare

**Desktop** (same on every desktop OS): click to place, drag a range, double-click a word, triple-click a paragraph, click then shift-click. Arrows move; shift extends. Option/Ctrl jumps by word, Cmd/Home/End by wrap line, Cmd+Up/Down by document — the usual modifiers.

**Up/down** follow **layout x**, not a character offset from the start of the line, and survive short and empty lines. That is how rich-text editors work.

**Phone:** tap places the caret, unless the tap is on the caret (or similar) and should open the menu instead. A range is usually started with a double-tap (or the platform equivalent) and adjusted with handles. Tap the selection for the menu. Match **Apple Notes / Bear** on iOS (Bear won an Apple Design Award). Match the prominent notes and text-editing apps on Android. Integrate those systems where it makes sense; don’t invent a third mobile selection language.

## Richer content

Images, link previews, and other non-text are still being settled. Wiggle room is expected; do not freeze today’s behavior.

We generally **pretend we are a rich-text editor**. The selection should always be visible. Placing the caret beside an image and backspacing should delete the **image as a whole**, not one character of its markdown. Same family as [layout](layout.md): an image is a glyph.

List markers, fold chips, and the rest of “pretty instead of source” follow that same rich-text instinct, tuned iteratively.

## Jobs

**Note and composer:** full selection, standard fare.

**Show:** no caret ([identity](identity.md)).

**Read-only:** you should still **select and copy**. Today that is believed missing — a gap, not a decision.

**Single-line:** the same jumping (word, and the rest) should work. Some of our single-line fields (possibly the Glyphon ones rather than `MdEdit`) are missing word-jump; that is a product gap.

Restored caret on reopen is a nicety, not this model ([document-model](document-model.md)).

## Not this note

- The `Region` / `Advance` enum, grapheme vs glyph as a buffer unit ([document-model](document-model.md)).
- Host keyboard / IME plumbing — later. The *feel* of native iOS and Android selection belongs here; the bridge does not.
- A locked list of every modifier chord. Native on that platform is the requirement.
