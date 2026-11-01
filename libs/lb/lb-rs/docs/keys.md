# Keys
Every file, links included, has its own AES-256 key, stored on the file wrapped under its parent's key. Holding a folder's key unlocks everything beneath it and nothing above. Bodies are compressed then encrypted under the file key with a fresh nonce; names are encrypted under the file key and hashed under the parent's. The root wraps its key under itself and is reachable only through a share to its owner.

## The account key
An account is a secp256k1 private key, shown to the user as 24 words (256 bits and a 4-bit check). It signs every request and every record, and it is half of every share: a share wraps the file key under an ECDH secret between the sharer's private key and the recipient's public key. There is no escrow and no recovery; losing the phrase loses the account.

## Shares are keys
A share is one entry on the file: mode, who encrypted it, who it is for, the wrapped key, and a deleted flag. Revoking marks the entry deleted and leaves the file key as it was; the server stops serving the file to the former recipient. The root carries a self-share — the root's key wrapped to the account itself — and that is what owning means to the chain.

## Decryption is lazy
To read a file, walk up until you reach a key you already hold or a share for you, then unwrap downward and cache each key. Nothing above the first share is touched, which is how a sharee reads a subtree whose ancestors they never see. The cache lives for the process and is never invalidated; a file's key never changes, so that is safe.

## What the server can and cannot do
It sees every structural fact and every share edge, compares names within one folder, matches document hashes, and can neither produce nor check a plaintext. Request authenticity is the signature on the request wrapper, within a clock window.

## Equality is lossy
Two records compare equal ignoring the wrapped file key, the name ciphertext (only its hash counts), the share ciphertexts, and the signature and timestamp. That equality decides whether a local change exists and whether a push's "previous record" matches the server's, so re-encrypting a key under a fresh nonce is not a change.

## Rough edges
- No key rotation on revoke or on moving a file out of a shared folder: a former recipient can read any later ciphertext of that file they obtain (#2165).
- The per-record signatures are carried and stored but verified nowhere — not on upload, not on pull — so a client trusts the server about who changed what (#775). The opsec FAQ says clients verify what they receive; the code does not yet.
- Ignoring the wrapped file key in equality is marked "verify intentional" in the code.
