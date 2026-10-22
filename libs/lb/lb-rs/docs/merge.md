# Merge
Merge rebuilds local on top of remote. It replays each local operation — create, move, rename, share, edit, delete — against the newly pulled tree, validates the result, and when validation fails records a constraint and rebuilds from scratch until it passes. The outcome is a local the server will accept and a base equal to remote.

## Who wins
Remote wins a concurrent move or rename: the client whose change reached the server first keeps it. A deletion on either side wins over any edit. Shares merge per sharer-and-recipient pair: a higher mode on either side is kept, and the merging client's own deleted-or-not state wins. For text, overlapping edits keep the *local* words; for drawings, remote wins per element; chats take the union of new messages, and a message remote removed stays removed.

## Content
Three kinds merge: markdown and plain text by a word-level operational-transform merge that never writes conflict markers; SVG by element; `.chat` by message. Anything else is duplicated — the local body becomes `name-1.ext` beside the original, which takes the remote body. The duplicate's id and key are chosen once per conflict and reused across rebuilds. Identical edits on both sides are not a conflict.

## Repair
[Validation](validation.md) names the violation and merge answers it: a cycle unmoves every locally moved file in it; a path conflict renames the local file `name-N.ext`, preferring a conflict duplicate; a link whose target you now own undoes your move if your move is what made it yours, else deletes the link; a link inside a newly shared folder removes the folder's shares, else deletes the link; a duplicate or broken link is deleted. Each answer must add a new constraint or the sync fails rather than loop. Orphans, children of non-folders, owner mismatches, over-long or undecryptable names, root changes, and edits to deleted files are not repaired; they abort the sync.

## Order
Cycles resolve before path conflicts because unmoving changes who is a sibling of whom; content duplicates are created in the pass that detects them, so their renames land on the retry. Base, local, and remote each satisfy the invariants alone, so every violation involves a local change, and local is what repair touches.

## Rough edges
- Remote wins structure and local wins prose. That is what the code does; it has never been written down as the rule.
- A conflict duplicate gets a random id and key, so two devices resolving the same conflict produce two different duplicates until one of them syncs first.
