# Document model

This is the building block under every job in [identity](identity.md). It was built early, with a long view of what it would have to support, and it held up. It also has design flaws. Maintenance has been delayed because it sits so deep that a fix cascades through the stack before the user feels any benefit.

## Source of truth

The document is the **bytes on disk**. They are compressed and encrypted; lb-rs undoes that on load and redoes it on save. We work in the cleartext. Parser state is never the source of truth.

Open then save must not modify the file. This editor is not the only writer: other Lockbook clients, Vim, an agent on a remote machine, other tools and processes. Sync will pull those bytes and incorporate them into whatever we have typed since. A plain file maximizes compatibility, keeps low-level and developer-oriented decisions obvious, and is especially convenient for AI — everything truly is just markdown (or plaintext, or code).

Workspace sync carries the file contents plus limited metadata (where it is, what it is called, who has access). It does not carry freeform metadata, comments, cursor positions, or the like.

Scroll offset and selection are restored as a nicety from a local workspace persistence file. They matter to the user. They are not the document. Toolbar layout persistence is not the document either.

## Encoding and “a character”

We own one encoding. Anyone else (iOS UTF-16, and so on) gets a translation layer. We do not support multiple encodings inside the model.

Positions in the buffer are Unicode segments (grapheme clusters) today. That was attractive because it avoids panicking on a mid-character index. After more of the editor existed, it became the wrong model. When the user arrows left or right they mean a **glyph**, which is a function of the font, not of the raw bytes. Baking Unicode segments in at this layer is a questionable design decision. Experimentation leans toward working in the bytes themselves; that is not locked.

Merge must not take two valid UTF-8 strings and produce invalid UTF-8. That can be enforced in the merge routines without making graphemes the unit of the whole stack.

There is no floor on how small a change can be. A keystroke, a mobile keyboard replacing a word, find-and-replace, an agent’s edit tool, a sync that replaces a single byte — all of those are edits.

`String` vs rope: no product opinion. Applying edits is not where we hurt; pursue the actual performance problems first.

## Many writers

The note must not crash. All writers’ views must converge to a common state.

Merge is a **word-level** diff. A byte-level merge of two valid UTF-8 documents can yield invalid UTF-8. A character-level diff of two words that share letters interleaves them into a nonsense term with no clear parent. Word-level at least keeps the words someone typed intact, even if only one side or both appear. We have the luxury of retuning to character, line, sentence, or paragraph if that ever feels better; words are the correct model for now. Overlapping-replace conflict behavior is in a decent place; we reserve the right to change it when something else makes more sense for the user.

North star: Google Docs–like collaboration without presence or moving carets. Other people’s edits just show up. If they land off-screen, you might not notice, and that is fine.

Save while typing: you should not notice. Reload with no external writes: you should not notice. Reload that pulls in another window, tab, or client: incorporate it without holding you up.

## Undo

The user should feel free to try something, knowing Cmd+Z puts them back. A toolbar button or shortcut that does a compound action undoes as one gulp. Typing is whatever feels good — usually a pause splits units; rapid characters grow into one unit that is not locked in when they were typed. Grouping is decided at **interpretation time**, when undo is invoked, from the facts at hand.

Track whatever information that decision needs.

Collaborative undo does not work today. The comments in the buffer about “remove a local op from the middle of the chain” are no longer the target. The promising direction: **never remove an operation; only roll forward.** To undo, a client pretends that one of *its* operations was never in the log, plays forward, and diffs current vs that state to produce a compensating event. Each client tracks its own operations so it does not undo someone else’s. Peers need not share undo stacks. The algorithm is still aspirational; this note does not freeze it.

## Versioning (near the editor, not the document)

A three-way merge needs ours, theirs, and a previously agreed base. Something associated with the editor has to know: have other clients’ changes already been incorporated? If we save, has the file changed on disk since? HMAC and that bookkeeping live there. Concurrent saves are a workspace/task problem, not this model.

## Folds

Folds live **in the document**, as an HTML comment in a fixed position (Bear-like). That was what it took to ship them. Alternate designs were not workable. There are aspects not to love (see [reveal](reveal.md): they are shared globally, better as reader-local). Users liked them too much to drop. They are here to stay.

Editor chrome is **drawn from** the document. Chrome does not live in the document.

## Open to not using this everywhere

This buffer is the text stack’s document model as far as it needs to be. Simpler jobs (nothing to edit, no content hash) need not take it. It got pulled into everything; that is not a requirement that they all keep it.
