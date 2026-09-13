# Folding

Users fold to **hide writing they consider done**, so it stops distracting from what they are focused on now. Headings and lists are enough for that. Other blocks would be nice with better disclosure ([blocks](blocks.md)); not required.

The `···` chip as a stand-in is [rich inlines](rich-inlines.md). Caret in folded contents **actually unfolds**; find **peeks** without unfolding ([reveal](reveal.md)). In the file as an HTML comment, Bear-like, copy-paste with Bear ([document-model](document-model.md)). **Reader-local is the desirable end state.** We have no workable design that gets us there, so the in-file tag is fine for now. Read-only: toggling is viewing, don’t save ([identity](identity.md)). Not important in composer or sent messages.

## How you fold

**Desktop:** hover to show the button, then click. Hover isn’t available on mobile.

**Mobile:** showing the buttons all the time forced **materially larger margins**, which steals screen from the actual note. That’s a frustration without a shipped solution. **Long-press to fold** is a good idea.

Users mostly click the buttons. We also have **keyboard shortcuts** to fold/unfold, including **mass** fold/unfold, which is how nested “unfold all” gets done.

Disclosure rules are **not settled**. Buttons clutter an otherwise clean page.

## The chip’s two jobs (on purpose)

Caret to the **right of** `···` sits between the tag and the hidden contents. Return there **inserts a new sibling** — a new list item, or another heading after a folded heading. That is intentional: otherwise making the next item is hard, and a non-heading after a folded heading would land *inside* the fold. Practical constraints, not an accident.

Replace / delete / copy the chip acts on the **contents** (rich-inline “select as a unit”).

## Replace

No strong opinion. Could **peek like find** and leave the fold in place. Don’t freeze more than that.

## Not this note

- The HTML comment string, parser sourcepos, fold-button hit targets.
- A locked hover-vs-always-visible spec. Desktop hover and mobile long-press are the current lean; disclosure can still move.
