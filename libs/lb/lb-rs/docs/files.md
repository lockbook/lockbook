# Files
A file is a signed metadata record: id, type, parent, name, owner, deleted flag, document size and hash, the keys that grant access, and the wrapped key that unlocks it. A document also has a body, stored and transferred separately and addressed by `(id, hash)`.

## Fixed and mutable
The id is a client-chosen UUID; the server refuses a new file whose id exists anywhere and refuses any diff that changes one. Type — document, folder, or [link](links.md) — is set at creation. Everything else moves: parent, name, owner, deleted, size, hash, access keys, and the wrapped key, which is rewritten on every move because it is encrypted under the parent's key.

## Root
Each account has one root: a folder whose parent is itself, named after the username, carrying a single self-share. It is created client-side at signup and cannot be renamed, moved, deleted, or shared. A tree may hold other roots — one per owner whose files you can see.

## Names
A name is encrypted under the file's own key and hashed under its *parent's*, so the server can detect two siblings with the same name and nothing else; moving a file re-derives its name. Names are non-empty, contain no `/`, and fit in 230 bytes. `name-N.ext` is the shape of a conflict rename; the number goes before the extension so the file keeps its type.

## Owner
Owner is stored per file and is always the parent's owner for a live file. A move sets the moved file's owner and every live descendant's to the new parent's owner, which is how a file moved into someone's shared folder becomes theirs; deleted descendants keep the old owner. The server reads owner for everything: whose tree a file is in, whose cap it counts against, which shares are pending, and which links are to your own files.

## Timestamps
`last_modified` is the signing client's clock at the moment it signed and `last_modified_by` is the signer, resolved to a username after sync. Both are display. The server stamps its own receipt time on each record and orders [sync](sync.md) by that.

## Size and hash
`doc_size` is the ciphertext length — what is stored and what is billed — and the server checks it against the uploaded body. `doc_hmac` is a keyed hash over the plaintext: a content address the server cannot invert or compare across users. A file with no hash has never had a body written.

## Deleted
`is_deleted` marks an explicit delete and is never cleared. A file under a deleted ancestor is implicitly deleted — computed, never stored ([deletion](deletion.md)).

## Rough edges
- The body hash is never checked on read, by client or server; a corrupt or substituted body is detected only when decryption fails.
- Client timestamps and signers are unverified, so `last_modified` and `last_modified_by` are whatever the writing client claimed.
