# Deletion
Deleting marks one file. Everything under it is implicitly deleted — derived by walking parents on each read, cached per tree, never written. The server does the same: it stores the explicit flag and recomputes implicit deletion when it needs to know.

## Permanent
A deletion cannot be undone and wins any conflict: a file edited here and deleted there is deleted. Edits to a deleted file are rejected locally and by the server. Deleting a folder deletes its contents as of the deletion's arrival at the server, not as of the click: a child moved out by another client that synced first survives. First to sync wins.

## What is reclaimed
Bodies, on the server. A deleted or overwritten body is scheduled and removed by a worker five to six minutes later; a deleted document's body is refused immediately. On the client, cleanup after every sync deletes bodies that no record references. Metadata is kept, short of deleting the account, on the server and on the client.

## Pruning is membership
On every sync the client asks for the full set of ids in its server tree and drops everything it holds that is not in the set, with descendants. That is how a share you declined or one withdrawn from you disappears, and it is the only path by which metadata leaves a client.

## Rough edges
- A deleted record still references its body, so a deleted document's body stays on the device for as long as its record does — and nothing prunes a deleted owned record, on the client or the server (#4369).
- An overwritten document's previous body stays fetchable by anyone who can see the file until the worker runs.
- Instant delete — reclaiming on the click rather than after sync — has been wanted since 2021 (#913).
- The 2021 design had the server materialize implicit deletions so clients could prune; the system settled on derivation and the pruning story above instead.
