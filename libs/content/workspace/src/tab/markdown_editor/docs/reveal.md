# Reveal
Obsidian proved that users can edit and preview in one UI by selectively hiding markdown syntax. While editing, the user is shown a rendered preview of the document except where the syntax is relevant to edits in progress. This is especially pleasing for people used to Google Docs or Microsoft Word, and for people still getting used to markdown — the punctuation is distracting, unaesthetic, and makes the editor feel like it isn’t for them.

Reveal and the toolbar together teach markdown: someone new can click Bold without knowing the syntax; they see that it added asterisks around the text, and they have learned a bit of markdown. Users report opening the welcome doc, feeling overwhelmed by the punctuation, then using the app via the toolbar and becoming comfortable.

## Properties
The conditions under which different parts of syntax are revealed have evolved with a general trend toward revealing less often as we trust the editor more. The target state is discussed in terms of a few overall properties rather than a specific list of conditions.

**You always should be able to see the cursor.** That means if the selection sits entirely inside syntax, show that syntax. A cursor at the edge of syntax does not necessarily count. If the selection extends across syntax, or has one end inside and the other outside, it can still be workable to keep that syntax hidden.

**A markdown node should always have a presence in the rendered note.** An empty heading should not hide its syntax, which would hide the whole empty heading.

**Showing syntax is for editing.** There is no need to show it for a readonly view like search result previews or sent chat messages.

## Layout stability
We aim to minimize the extent to which hide/reveal transitions re-arrange content. The cost of revealing liberally is proportional to how much the layout will change. In a mild example, clicking inside an italic section reveals two characters. In a severe example, clicking inside a link reveals the link URL. Revealing many characters can shift text layout so that the resting position of the caret after a click is quite far from the click position, for example.

Block nodes and rich inlines (like inline images and link previews), which have a greater layout shift upon reveal, deserve more conservative syntax reveal.

We experimented with a hover reveal mechanism where syntax would reveal on hover. This had the desirable property that by the time you clicked, syntax was already revealed in accordance with where you are about to click, so that the cursor always lands where you click. However, the mechanism failed. Hover could re-layout a paragraph so that the hover position is no longer over the node, which undoes the reveal that caused it. The editor then flickered jarringly between the two layouts until the hover position changed.

While drag-selecting, reveal follows only the committed selection. This addresses issues similar to hover; there's a cyclical dependency between the position you've dragged to and what's revealed. This feature resolves the cycle by only updating the reveal when the drag-select stops, a solution that has no equivalent for hover reveal.

## Folds
We support [folding](folding.md) content sections as an additional, explicit hide/reveal feature.  A fold hides or shows a large region — often many blocks, sometimes many viewport heights — presenting an extreme case of layout instability.

If the cursor is in a folded section, we unfold it for real, so there's no jump when they move the cursor away. There's nowhere to click that places the cursor inside the folded section. Combined, this means that a cursor placement will never toggle a fold.

If a folded section contains a find match, we unfold it temporarily. Iterating matches should not leave many sections of the document unfolded and this workflow involves content moving rapidly across the screen already.