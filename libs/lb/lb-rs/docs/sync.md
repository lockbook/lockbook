# Sync
Sync settles the client against the server in three steps: pull, push metadata, push content. Pull fetches every record changed since the client's `last_synced`, fetches the bodies it needs, [merges](merge.md) remote against base and local, writes the result, then advances `last_synced`. Push sends every local record as one batch, then each changed body. Nothing is read back from a push; the client sees its own pushes on the next pull.

## When
On a local edit. Every 3 s while the user is active, every 5 min otherwise, and on return to the foreground. Two syncs on one client never overlap; local operations interleave between sync's transactions and never wait on the network. After each sync, successful or not, a worker prefetches the notes and drawings you do not yet hold, through links into shared folders.

## One clock
The client keeps no per-file versions. Its only clock is `last_synced`; content is identified by hash. The server stamps each record with its receipt time and answers "everything stamped at or after your `last_synced`", plus every descendant of any shared file in that set. `as_of` is the server's time at response.

## Preconditions
A push carries, for each record, the exact previous record the client believes the server holds. Any difference — a concurrent rename, a share added elsewhere — rejects the whole metadata batch, and a body push is rejected if its record changed on the server in any way. A client's own pending rename and pending edit do not block each other; anything on the server it has not pulled blocks both. A rejected sync is not retried inside the sync; the next tick pulls, merges, and pushes again.

## Partial progress
Each stage commits: merged state before `last_synced`, accepted metadata before bodies, each body on its own. A sync that fails halfway leaves a consistent client that resumes where it stopped.

## Lazy files
Pull fetches a body only if the client already held the previous one or has local edits to it. Everything else arrives on first read, and the prefetcher fills in notes and drawings behind. A large file or a large shared folder no longer takes sync offline; a document you have never opened costs nothing until you do.

## Rough edges
- The edit-triggered sync is meant to debounce at 500 ms. As written, a lone edit on an idle channel syncs at once and a burst of edits does not trigger it at all; the 3 s worker covers the gap.
- The inclusive bound re-delivers the boundary record every pull, and because `as_of` is the response time rather than the newest version returned, a record stamped before its body landed can be skipped for good (#5156).
- A precondition rejection surfaces to the user as "unexpected sync problem" until the next tick clears it.
