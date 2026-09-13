# Reveal

Hiding markdown syntax is how edit and preview live in one view. The user should not leave the editor, hit a button, or change a mode to see the document as it will look in the app or anywhere they publish it. They should already be looking at that. That is especially pleasing for people used to Google Docs or Word, and for people still getting used to markdown — the punctuation is distracting, unaesthetic, and makes the editor feel like it isn’t for them. Obsidian proved the use case.

The file on disk is still the markdown. Hiding is presentation.

Reveal and the toolbar together teach markdown by accident. Someone new can click Bold without knowing the syntax; they see that it added asterisks around the text, and they have learned a bit of markdown. People report opening the welcome doc, feeling overwhelmed by the punctuation, then using the app via the toolbar until they are comfortable. That path is an effective feature, not a side effect. Applying a style from the toolbar should make the syntax visible enough in that moment to learn from — a reason to show source that is not “the caret would vanish.”

## Stance

The exact conditions under which source shows are not a frozen list, and they are not quite right today. They have evolved a lot. What we need is a design that lets us experiment and fine-tune without a rewrite.

Direction of travel: **hide more as the implementation gets more trustworthy.** Early on we revealed when in doubt, because bugs made the document hard to understand unless source was visible. As it got more robust, hiding in more situations made users happier. New constructs should start from that later, more conservative end — not from “reveal when unsure.”

The `reveal_ranges` accumulator is a workable implementation, not a requirement.

## The rule that held

You should be able to see the cursor.

If the selection sits entirely inside hidable syntax and does not touch its edges, show that syntax. Otherwise there is nowhere to draw the caret, which is confusing.

Same idea: an **empty ATX heading** must still show its `#`s. If we hid them, the block would vanish and the caret would have nowhere to sit.

If the selection extends across syntax, or has one end inside and the other outside, it can still be workable to keep that syntax hidden.

Reveal is motivated by the cursor. Where markdown is not editable, we usually show no source.

## Layout stability

Showing syntax must not yank the document around.

Mild case: clicking a heading reveals `#`s, the line wraps, text shifts a little.

Killer case: links. The destination is often long and the preview short. Revealing rewraps a whole paragraph. You click to put the caret near a link and it comes to rest far from the click because the layout rearranged.

The cost of a liberal reveal is proportional to how much layout will change. Be conservative wherever revealing expands the line a lot (links, images, cards, and other non-text inlines). Bold and similar inlines jump much less, so they can be less strict.

### What we tried

**Hover-to-reveal** so a click would land on already-revealed layout (no jump between hover and click). Failed: hover reveals → paragraph rewraps → the pointer is no longer over the node → it unreveals → flicker, sometimes every frame.

**In-progress vs committed selection while dragging.** Reveal follows only the committed selection; the drag’s live range does not. Otherwise the point under the pointer, the selection, and reveal fight as layout jumps. This is the target everywhere. iOS does not have it yet (virtual-keyboard difficulty).

Unfocused: it is nice for the editor to look fully previewed. Not worth extra complexity if we already do, or almost do.

Find: the match must be visible. Search preview: showing the previewed snippet is enough.

## Block markers vs inlines

“Gutter” is the wrong word (that’s line numbers / diffs). Here we mean the syntax that comes with blocks — list markers, quote prefixes, and the like.

Those are revealed more strictly than most inlines, driven by user feedback on jumping. Inlines like bold are not that severe. Links still are: placing the caret merely beside a link, or selecting a paragraph and having every link expand, rewraps the paragraph and you lose your place.

Images, cards, and other non-text inlines are the same family — conservative because layout change is large — not a second concept. They exist because wrap layout had to host things that are not text.

Code-fence backticks: open to tightening the conditions to reduce jumping, now that we trust the implementation more. Details for the blocks note.

## Folds

A fold hides or shows a large region (often many blocks, sometimes many viewport heights). Unfolding when we don’t have to is the same jumping problem at a larger scale.

If the cursor is in a folded section, unfold it for real. Peeking would hide the cursor; leaving it peeked would jump a second time when the cursor leaves. We showed both to users; they preferred actually unfolding.

That unfold is technically an edit: fold state is in the file and shared across clients. That is design debt. Folding is better as something that applies to the *reader*, not globally to everyone looking at the document. Same tension as allowing fold toggles on a read-only shared file (viewing, refuse to save) — see [identity](identity.md).

Do not unfold in unnecessary circumstances.

## Not this note

- Spoilers (tap to see hidden text) — a different feature.
- A complete table of per-construct conditions — those live with blocks, inlines, and folding, and should stay cheap to change.
- Side-by-side source and preview — the source side is a plaintext editor, not “reveal everything.” See the plaintext/code job in [identity](identity.md).
