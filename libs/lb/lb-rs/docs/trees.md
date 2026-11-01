# Trees
A tree is a map from id to signed record with derived views over it. The client holds two: `base`, the last state agreed with the server, and `local`, the records that differ from base. The working tree is base with local staged on top. An unsynced change is exactly an id with a record in local.

## Copy on write
Staging a record equal to its base record removes it from local instead of storing it, and after every merge and push local is pruned of anything equal to base. Rename a file and rename it back and there is nothing to push and nothing dirty. Equality is the lossy kind ([keys](keys.md)).

## Staging
Every operation is a small tree staged over the working tree — one record or a few — validated, then promoted into local. The syncer stages larger ones: remote over base, then local operations replayed over that, promoting to base and local separately. Removal is staging too: a staged removal hides a base record without touching it, and promoting it deletes the record.

## Lazy views
Decrypted names, implicit deletion, children, and the target-to-link map are computed on first use and kept for the life of the tree; any staging or promotion discards them. Access mode, pending roots, descendants, and ancestors are recomputed on every call.

## Roots and orphans
A parentless file is a share root if it carries any share key, and the cycle and deletion walks stop there; the access walk stops at any parentless file. Validation and pull are stricter: a parentless record with no key for you is an orphan, rejected locally and dropped on arrival. One root per owner is allowed; a second for the same owner is reported as a cycle.

## The server's tree
The server keeps every record ever uploaded, short of an account being deleted, indexed by owner and by sharee. An account's tree is the files it owns, the files shared to it, and all descendants of those. Anything outside is not forbidden but absent — lookups find nothing — and that set is what `GetFileIds` returns and what the client prunes against ([deletion](deletion.md)).
