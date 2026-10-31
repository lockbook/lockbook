# Validation
One function, run for every local operation, inside merge, on the server for every upsert and content change, and by the integrity check. It works on encrypted records — hashes, ids, public keys — and decrypts nothing, so an invariant violation never surfaces as a decryption failure.

## The checks, in order
Root unchanged, and no file becomes its own parent. No edits to deleted files. Encrypted names within 254 bytes. Every file has a present parent or a key for the user whose tree this is — the orphan check, which is what admits share roots. Only folders have children. A file's owner equals its parent's. No cycles, and one root per owner. No two live siblings with equal name hashes. No shared links, duplicate links, broken links, or links to your own files. Then, per changed field, that the actor had write access at the file's *previous* parent, so a move cannot grant itself the access it needs.

## Owner equals write
Validation does not distinguish owner from write. Otherwise you could move a file from a write-shared folder into your own, do something only owners may, and move it back.

## Who repairs what
[Merge](merge.md) repairs cycles, path conflicts, and the four link rules; every other failure aborts the sync. Locally each failure is an error with a message; the server returns it as a typed rejection.

## Outside the function
Names are checked for emptiness and `/` on create and rename. The server adds: the previous record equals the current, the id is unchanged, hash and size are unchanged in a metadata push, new ids are globally unused, and the owner's usage cap ([usage-limits](usage-limits.md)).

## Rough edges
- Access is read from the staged tree, keys included, while the rationale comment says base. Whether a read sharee can stage a write key for themself and use it in the same batch is untested.
