# Consistency
Metadata lives in a db-rs log; one transaction is one durable append, all or nothing. Bodies are files beside it named `id-hash`, written to `.pending` and renamed into place. A body and its record are never in one transaction, so order carries the guarantee: body first, record second, `last_synced` last. A crash leaves at worst an unreferenced body, reclaimed by cleanup, or records ahead of `last_synced`, re-pulled harmlessly. It never leaves `last_synced` ahead of what was stored.

## Writing a document
`safe_write` is compare-and-swap on the hash: it fails if the record's hash is not the one you read, encrypts with no lock held, then renames and updates the record in one transaction after checking the hash again. Editors are never locked for sync. When sync writes a body an open tab is showing, the tab reloads and merges the disk text into its live buffer with the same word-level merge; a save that loses the race reloads and tries again. Agents' edit tools loop on the same mechanism.

## Merge under lock
The whole merge, including decrypting and merging every conflicted body, runs inside one write transaction, so a local write cannot land between the merged state and its commit.

## The server's write
Metadata is one global lock over a db-rs log; validation precedes the first mutation, so a rejected batch leaves nothing behind. A content change is two-phase: validate under the lock, write the body with the lock released, re-validate and promote under the lock, and delete the new body if the second phase fails. The previous body is scheduled for the garbage worker rather than deleted, so a client fetching the old hash during the window still succeeds.

## Recovery
The server log compacts on a long window and has no rollback. Recovery from a bad write is manual: find the byte at which the log went wrong, truncate, and have any client that pushed past it clean-sync. The long compaction window is what keeps that possible.

## Rough edges
- Merge holds the write lock for its whole duration and warns past 100 ms; a large conflicted drawing holds it longer. Merging outside the lock with a final compare-and-swap is the direction.
- Server bodies are written without fsync or a temp-and-rename.
- Tooling to read and edit the server log, and a maintenance mode that clients recognize, are still owed.
