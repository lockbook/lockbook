# Events and status
lb-rs broadcasts and clients react; clients never poll, and lb-rs itself runs the periodic sync. The events are `MetadataChanged(actor)`, `DocumentWritten(id, actor)`, `PendingSharesChanged`, `Sync(increment)`, `StatusUpdated`, and `UserSignedIn`. The actor is the user or sync; a document write also carries the originating tab, so a tab ignores its own write and acts on everyone else's. Sync's own writes never trigger another sync.

## Increments
Started; pulling or pushing a named document, begin and end; finished with an optional error. Metadata push has no progress event. Finished fires before cleanup runs.

## Status
One struct recomputed on events: offline, syncing, out of space, pending shares, update required, pushing, dirty locally, pulling, space used, last sync, unexpected problem. Its one-line message says, in order of precedence: syncing, offline, out of space, update required, unexpected problem, how many files are dirty, when you last synced. Pending shares, in-flight documents, and space used are for clients to show on their own. Dirty is the set of ids in local. Usage refreshes at most once a minute. Three sync errors are distinguished — update required, offline, out of space — and everything else is unexpected.

## Rough edges
- `PendingSharesChanged` fires on any pulled change owned by someone else rather than on a change to the pending set, and status ignores it, recomputing pending shares only when a sync finishes.
