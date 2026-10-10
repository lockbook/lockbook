# API
Twenty-odd authenticated endpoints under one request wrapper, a few unauthenticated routes, and one rule: the server validates structure and authority, never content.

## Shape
A request is signed by the account key over a timestamp. The server gates on a minimum client version with `ClientUpdateRequired`, then checks the signature and a clock window. Bodies are JSON or bincode by header. There is no request size limit; a 1 GiB document is one body.

## The ones that matter
`upsert-file-metadata-v2` takes a batch of (previous, new) record pairs and applies all or none, validating the whole resulting tree. `change-document-content-v2` takes one record diff that changes only the hash, plus the body. `get-updates-v2` takes `last_synced`. `get-document` takes `(id, hash)`. `get-file-ids` returns the whole visible tree. `get-usage`, `new-account-v2`, `get-public-key`, `get-username`, and `delete-account` round out what clients call; billing, admin, debug upload, `/open/<uuid>`, and the Apple site association are the rest.

## Errors
Each endpoint has a typed error enum. On the sync path the client passes three through to [status](events-status.md) — update required, over cap, unreachable — and folds the rest into `Unexpected`. Account and username lookups keep their typed errors.

## Never read back
Upsert returns nothing; the client sees its writes on the next pull. The content endpoint returns nothing; the hash was the client's to begin with.

## Versioning
Reworked routes carry a `-v2` suffix. The record is a versioned enum (`Meta::V1`) so fields can be added without a wire break, in service of document history (#190). Feature flags exist only on the server — new accounts, account limits, bandwidth controls — and are not exposed to clients.

## Rough edges
- Folding sync errors into `Unexpected` means a precondition rejection reads as "unexpected sync problem".
- `admin-rebuild-index` is the one admin route with no admin check.
- `new-account-v2` inserts the submitted root without checking that it is a root, owned by the signer, or validly signed.
