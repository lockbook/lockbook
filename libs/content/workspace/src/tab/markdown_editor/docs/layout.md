# Layout

How the document sits on the screen. [Reveal](reveal.md) already owns “don’t jump when syntax appears.” This note is wrap, the viewport, and staying put. Caret rules are selection (next). Keyboard/IME later.

## Two kinds of line

A **source line** is a run of the file delimited by newlines (carriage returns accounted for). Unobjectionable.

A **wrap line** (a row) is whatever layout placed in one horizontal strip.

The user does not scroll by either. They scroll in points — mouse wheel, two-finger trackpad, and the like. Some platforms *submit* line-sized wheel events; we still process them in our own terms.

[Dialect](dialect.md): source newlines render as hard breaks. Wrap is what happens *inside* a source line when it is wider than the column.

## The page

A markdown note always wraps to the window. Past a **max width** we pad the sides so the column feels airy rather than stretching with the display. How wide that column should be is contended; we are likely to offer a setting. Until then there is one default.

Horizontal scroll does not make sense for the markdown document as a whole, and nobody has asked for it. Users *have* asked for horizontal scroll **inside code blocks**, still within the document’s bounded width. Tables likely want the same. Neither is shipped; both are reasonable.

## Not-text in the flow

Inline images should feel like a **glyph**: select, edit, copy — the things you’d do in a rich-text editor — without revealing the image’s source. We are still finding the exact feel; glyph is the right model.

A **bare autolink** on its own line renders as a **card**; inside a paragraph it is a wrappable **chip**. Labeled `[text](url)` stays text. See [rich inlines](rich-inlines.md).

When an image finishes loading and the paragraph gets taller, that is the same family of jump as reveal: we like to avoid it. It is also acceptable. There is no workaround for the first load. Caching sizes so we can reserve space without decoding is best-effort.

## Viewport

A long note must stay usable. We aspire to **huge notes on very low-end hardware**. Only what is on screen needs to be fully laid out and painted; off-screen should be irrelevant to that frame.

The scrollbar and thumb are conceptually about the **height of the whole document**, which would otherwise mean laying out the whole document. Obsidian appears to approximate, refine as you scroll, and remember; the thumb can move out from under a drag, and they stop parsing around ~100K. Their speed is acceptable; those limitations are what our more complex scroll is working around. The complexity is justified.

What is *inside* the scroll vs around it (toolbar, find) is why the editor is layered — see the note vs composer jobs in [identity](identity.md).

Trailing room so the last lines can sit in the middle of the viewport, scroll-to-caret / find / keyboard, and “don’t yank scroll while you’re only reading” are the intended behaviors of that scroll. We are pleased with our scroll area implementation except that it is hard to implement and use.

**Stay put when width changes.** Rotate, split-screen, undock: you should still be looking at the same place in the document. Width-independent scroll is in a decent place.

## Building block

The virtualized scroll is meant to be general-purpose — a replacement for egui’s scroll-rows — and is already used elsewhere. Wrap + scroll are pieces other jobs compose, not “the note editor’s private layout engine.”

Jobs from [identity](identity.md) that change geometry: the note is a full scrolling document; the composer is short and may omit chrome that lives outside the scroll; show is just the height of the content; single-line is one unbounded line that follows the caret sideways.

## Wrap quality

The wrap implementation was built under time pressure. It still breaks and wraps in ways that feel counterintuitive. That is undesirable. Revisit on some timeline. Do not freeze today’s breaker as the requirement.

RTL, combining marks, and similar were motivated by **stability**: agree on invariants, property-test random documents until the editor stops crashing. Those accumulations prevent panics; they are not themselves layout requirements, and they are not a claim that we understand or promise a particular international typography.

## Not this note

- Neighbor rows, last-frame hit-testing, cache keys, affine slopes — host and implementation.
- First tap on a document that “hasn’t been laid out”: the document is laid out before it is shown; it renders essentially instantly. Not a product hole.
- Scroll offset / caret restored on reopen — a nicety, not the document ([document-model](document-model.md)); the *geometry* of staying put on resize is here.
- How a list marker or image *selects* — selection.
