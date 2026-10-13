# Document model
## Source of truth
The document is bytes, not the parser state. Documents are truly just markdown. Where the files are, what they're called, and who has access lives in file metadata. They are stored on the server, synced to local disk for offline access, and decrypted and decompressed on demand to load a file.

Viewing the file must not modify it and edits should preserve the source outside the intent of the edit. For example, we shouldn't format on save or load, shouldn't parse and re-render, and shouldn't renumber a list to insert or remove one element. Edits affect recent/suggested notes.

## Encoding and characters
Documents are encoded in UTF-8; platforms preferring other encodings work through a translation layer (e.g. iOS UTF-16).

When the user arrows left or right they mean a **glyph**, which is a function of the font, not of the raw bytes. Unicode grapheme clusters are used to represent offsets so indexes don't land mid-character, but using them for character steps is incorrect. Experimentation leans toward working in the bytes themselves more often with graphemes as a higher level abstraction where useful. For example, a merge routine for documents must not take two valid UTF-8 strings and produce invalid UTF-8, but it can meet this requirement without graphemes being the unit of all operations in the stack.

A keystroke, a mobile keyboard replacing a word, find-and-replace, an agent’s edit tool, a sync that replaces a single byte — all of those are edits.

## Write concurrency
Edits can come from the user keystrokes, IME, find and replace, AI tools, and other instances of our app or other apps working in the files directly via sync. The note must not crash and all writers’ views must converge to a common state.

Merge is a **word-level** diff, as a byte-level merge of two valid UTF-8 documents can yield invalid UTF-8 and a character-level diff of two words that share letters interleaves them into a nonsense term with no clear origin. Word-level keeps the words someone typed intact, even if only one side or both appear.

In the target state, other systems' edits just show up. Saves and reloads/merges are automatic and don't interfere with use. Presence indicators and other users' carets are nice to have.

## Undo
The user should feel free to experiment and rely on undo to protect them from undesired consequences. A toolbar button or shortcut that does a compound action undoes as one unit. For typing, undo does whatever feels good — usually a pause splits units; rapid characters grow into one unit until a pause.

Collaborative undo: **never remove an operation; only roll forward.** To undo, a client pretends that one of *its* operations was never in the log, plays forward, and diffs current vs that state to produce a compensating event. Each client tracks its own operations so it does not undo someone else’s. Peers need not share undo stacks.

## Versioning
A three-way merge needs ours, theirs, and a previously agreed base. Something associated with the editor has to know: have other clients’ changes already been incorporated? If we save, has the file changed on disk since? The relevant bookkeeping is ideally done in workspace rather than the editor.

## Folds and chrome
Folds live **in the document**, as an HTML comment in a fixed position (Bear-like). They are shared globally, which is worse than reader-local ([reveal](reveal.md), [folding](folding.md)), but design alternatives were unworkable and users liked them too much to drop.

Folds follow a general pattern that editor chrome is **drawn from** the document, with the document itself as the source of truth.