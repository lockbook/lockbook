# Folding
Users generally fold content to hide writing they consider done so it stops distracting from what they are focused on now. Headings and lists are enough. Other blocks would be nice with progressive disclosure to mitigate UI clutter ([blocks](blocks.md)).

While reader-local is a desirable end state, we have no workable design that gets us there. Folds are modeled as Bear-style in-document HTML tags.

**Read-only:** toggling is supported but we don't save ([identity](identity.md)).

## How you fold
**Desktop:** hover to show the button, then click. **Mobile:** always-visible buttons force materially larger margins; long-press menu to fold is a good idea.

Users mostly click the buttons. Keyboard shortcuts fold/unfold, including mass fold/unfold for nested "unfold all."

## The chip
Caret to the right of `···` sits between the tag and the hidden contents. Return there inserts a new sibling — a new list item, or another heading after a folded heading — otherwise making the next item is hard, and a non-heading after a folded heading would land inside the fold.

Replace / delete / copy the chip acts on the contents. Replace could peek like find and leave the fold in place.
