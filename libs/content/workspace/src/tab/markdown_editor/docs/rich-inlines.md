# Rich inlines

Inlines that are **not** the circumfix family ([inlines](inlines.md)). One family: an image, a link chip, a link card, a fold `···` chip. Each **stands in for source** you usually don’t want to look at. Conservative reveal, because showing the source jumps layout ([reveal](reveal.md)). Fetch, drop, camera, wiki resolution — [links-images](links-images.md). Folding *as a feature* is [folding](folding.md); the chip as a stand-in is here.

## One family

Select as a unit, like a rich-text glyph ([layout](layout.md), [selection](selection.md)). Click has a job (open, unfold, menu). Backspace deletes the whole thing, not one character of markdown. Copy should eventually be able to take **the pretty thing and the source** (images: the pixels *and* the `![]()`; not shipped).

Mobile “Edit” puts the caret in the hidden destination so you can change a URL without the stand-in vanishing for lack of an interior. Implementation name is irrelevant.

## Links: chip vs card

A **bare autolink** switches: **alone on its line** → a block **card**; **inside a sentence** → a wrappable **chip**. `[label](url)` stays labeled text (plus the usual underline). `<https://…>` opts out of the pretty form.

That switching, and the **link-fetching editor setting**, are the latest link behavior. We are happy with them for now. Fine to tune; don’t rewrite the idea.

Read-only: click **opens** ([inlines](inlines.md)). Editing chrome is **not decided**. A lightweight context menu feels right — Open / Edit / Copy — so title vs destination is less of a muddle than “caret on the URL.”

## Images

A glyph in the wrap. Obsidian `\|WxH` is a feature we mean ([dialect](dialect.md)).

**Alt text:** whatever is normal — likely hover, and a placeholder if the image cannot be shown. Not thought hard.

Copy the image *and* the source would be cool. Same family as copy-as-plain / copy-as-markdown ([edits](edits.md)).

First-load size jump: avoid when we can, acceptable ([layout](layout.md)).

## Fold chip

Closely modeled on **Bear**. Click unfolds. Select as one unit. Stands in for the hidden HTML comment and contents; **never show the tag source**.

Behavior is more or less settled; refer to the implementation, and to the folding note for reader-local debt and “cursor in fold → actually unfold.”

**Not important** in the composer or in sent messages. We would not show fold controls on a sent chat message. If someone pasted folded content and it rendered unfolded, nobody would notice. Rendering it folded would be kind of cool. Maybe the *whole* composer message is foldable rather than inner headings/lists. Edges exist; do not block composer on this.

## Jobs

**Note:** the full family.

**Composer:** images and link cards/chips (identity). Folds: skip.

**Show / read-only:** pretty form, click opens (links) or is inert (images — whatever is normal). No fold chrome.

## Not this note

- Circumfix styles, spoiler tap, math-as-image-like drawing ([inlines](inlines.md)).
- How we fetch titles, favicons, files, or paste a photo.
- List reorder.
