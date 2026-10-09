# Layout
A markdown note always wraps to the window. Past a **max width**, we pad the sides so the column feels airy rather than stretching with the display. How wide that column should be is contended; we are likely to offer a setting.

The markdown document as a whole does not scroll horizontally, but horizontal scroll inside code blocks, still within the document’s bounded width, is something users have asked for. Tables likely want the same.

## Blocks & Wraps
A markdown note is displayed as a vertical stack of markdown blocks. Blocks are spaced to visualize the blank lines between them, unlike markdown renderers without editing.

Wrap contexts are used for text, circumfix inlines which are essentially just styled text, and rich inlines like link previews, inline images, and math. Rich inlines should feel like single glyphs: select, edit, copy — the things you’d do in a rich-text editor — without revealing an image’s source. Bare autolinks render as chips in paragraphs or as full cards when they are the whole paragraph.

Handling of RTL, combining marks, and similar constructs is stability-driven as well as feature-driven. Users desire emoji and support for additional languages. Halfway measures, built under time pressure, deliver on stability without the target feature set as a temporary measure.

## Scroll Virtualization
A long note must stay usable. We aspire to huge notes on very low-end hardware. To that end, only what is on screen needs to be fully laid out and painted; off-screen should be irrelevant to a frame.

The scrollbar and thumb are conceptually about the height of the whole document, which would otherwise mean laying out the whole document. Obsidian appears to approximate, refine as you scroll, and remember; the thumb can move out from under a drag, and they stop parsing around ~100K. Their speed is acceptable; those limitations are what justifies our more complex scroll.

The virtualized scroll is general-purpose — a replacement for egui’s scroll-rows — and is already used elsewhere.