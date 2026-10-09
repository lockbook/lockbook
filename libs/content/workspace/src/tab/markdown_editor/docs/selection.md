# Selection
Selection should behave like normal on a given platform unless there's an exceptional justification.

**Note and composer:** full selection, standard fare.
**Show:** no caret ([identity](identity.md)).
**Read-only:** you should still **select and copy**.
**Single-line:** the same jumping (word, and the rest) should work.

## Data Model
The selection is an **ordered range**. One end is the caret (the end that moves with the arrows). The other is the far end — where a drag started, or simply the end that does not move.

There is at most one selection; we aim to support **zero** as well. Zero matters because selection drives [reveal](reveal.md). On iOS a newly opened note sits at the default `(0, 0)`. Notes often start with a heading, so the first `#`s appear — but the system caret is not shown until the field is first responder, so you have not tapped yet and still get syntax. The right model for “not editing yet” is **no selection**, not a hidden caret at the start.

Multiple selections are a nice-to-have as we are not an IDE.

## Rich content
We generally **pretend we are a rich-text editor** around rich inlines like inline images and link previews.  Placing the caret beside an image and backspacing should delete the **image as a whole**, not one character of its markdown. Same family as [layout](layout.md): an image is a glyph. List markers, fold chips, and the rest of “pretty instead of source” follow that same rich-text instinct, tuned iteratively - the exact feel of images, cards, and other non-text can still move.