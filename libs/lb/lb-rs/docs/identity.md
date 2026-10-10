# Identity
lb-rs is the one core under every Lockbook client: the file tree, its encryption, local storage, and sync against a single server. Apple, Android, desktop, the CLI, and the workspace call the same functions and see the same tree. The server decides nothing about a user's files; it stores what clients hand it and arbitrates who may change what.

## Local first
Every read and every edit is local. A client holds the whole tree it can see as metadata, plus whichever document bodies it has fetched. The network is for [sync](sync.md) and for a few lookups: a username's public key, a body you have not fetched yet. Very few actions need a connection; sharing is the notable one, because it needs the recipient's key. Sync is a background settlement, not a step the user takes.

## What the server knows
The server sees the shape: ids, types, parents, owners, deletion flags, sizes, document hashes, every share edge with its mode, when each record last changed, and which account signed it. It can test two sibling names for equality and two document hashes for equality, and nothing more. It cannot read a name, a key, or a byte of content. [keys](keys.md) says why the line falls there; `privacy.md` and `values.md` promise it to users.

## One tree, many owners
Your tree is your own files plus every subtree someone shared with you, each hanging from a root you can see whose parent you cannot. A parentless file with a key for you is a root; a sharee's tree is a forest ([trees](trees.md), [sharing](sharing.md)).

## Settlement, not realtime
Sync reconciles whole states: pull what changed, merge, push. It is built to be correct after any sequence of concurrent edits rather than fast. Realtime collaboration, if built, layers diffs on top and uses this as its compaction; lockless editing across devices already works through `safe_write` and reload ([consistency](consistency.md)).

## Permanence
Deletion is forever and wins over concurrent edits. Metadata is kept for the life of the account; only bodies are reclaimed. A client offline for a year syncs the same way as one offline for a minute ([deletion](deletion.md)).
