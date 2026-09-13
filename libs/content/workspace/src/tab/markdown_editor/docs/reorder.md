# Reorder

Drag a **list item** (and everything inside it) to another place in the **same list**. Lists only — not headings, quotes, or arbitrary blocks. Users asked; it belongs in the note and the composer ([identity](identity.md)). No extra chrome: progressive disclosure if you use it. Read-only: this is an edit (unlike fold). Hide the software keyboard while dragging is a nice-to-have, not a requirement ([ime](ime.md)).

## How you grab

**Desktop:** drag the **marker**. Hover changes the cursor to a hand — that is the only way to discover the feature. Task items feel a bit strange: click toggles the checkbox, drag on the same control reorders. Accepted for now.

**Phone:** **long-press**, then drag, so a pan still scrolls. We tried other gestures and didn’t like what some popular apps do. Landed somewhere we are pleased enough with. Quality misses maybe; the design feels good.

**Apple + hardware keyboard:** a finger-drag still **scrolls**, it does not start a text selection ([ime](ime.md)). Reorder stays marker / long-press, not a swipe-select.

There is **no keyboard-centric move** (no Alt-Down). Some editors have that. Copy and paste cover it for now.

## Where it lands

**Sibling-only** (same parent list). Currently accepted. A version that can indent, outdent, or drop into another list would be more fully featured — and a lot more implementation *and* UX complexity, which would be a step in the wrong direction.

Need at least one sibling or there is nothing to reorder.

## After the drop

The only lasting impact is the thing you moved. **Selection stays more or less as it was.** Caret and scroll stay put **except** while dragging: auto-scroll near the viewport edges (and on mobile while the drag is running). On release, don’t jump to the caret.

Not sure the current build matches that; the target is clear.

**Numbered lists:** the number we *draw* comes from position, so rewriting `1. 2. 3.` in the source often doesn’t matter. Would have to check. Don’t freeze a renumber pass.

Undo is one gulp ([document-model](document-model.md)).

## Feedback

A floating card is pleasant. Whatever reads as “I’m dragging this item” is valid (dim the hole, a drop gap, …).

## Jobs

**Note and composer:** same gesture, no extra UI.

**Show / read-only:** no.

## Not this note

- `TouchReorder` / `plan_block_move`.
- Multi-item drag of a selected sibling run — implementation can do it; not discussed as a product. Fine if it falls out of “selection stays as it was.”
