# lb-rs
Design of the file tree, its encryption, and sync, to support engineering.

Fundamentals:
- [identity](docs/identity.md) — one core, many clients; local first; the server sees shape, never content; settlement, not realtime
- [files](docs/files.md) — a signed record; id and type fixed, everything else moves; names hashed under the parent
- [keys](docs/keys.md) — a key per file, chained to the parent; shares are wrapped keys; revoke does not rotate
- [trees](docs/trees.md) — base and local; copy on write; share roots are tops; the server's tree per account
- [deletion](docs/deletion.md) — one flag, derived downward; permanent; bodies reclaimed, metadata kept
- [sync](docs/sync.md) — pull, push metadata, push content; one clock; exact-match preconditions; lazy files
- [merge](docs/merge.md) — replay local on remote; remote wins structure, local wins prose; repair by rebuild
- [consistency](docs/consistency.md) — one transaction per change; body before record; `safe_write`; the server's two-phase write

Sharing:
- [sharing](docs/sharing.md) — a share is a key; shared with me; accept is placement; the owner pays
- [links](docs/links.md) — a file whose content is an id; invisible; the id contract; two paths
- [document-links](docs/document-links.md) — paths within a tree, `lb://` across; the external URL

Supporting:
- [api](docs/api.md) — the endpoints, their preconditions, what the client never reads back
- [validation](docs/validation.md) — the checks in order; who repairs what
- [events-status](docs/events-status.md) — events, actors, the status line
- [usage-limits](docs/usage-limits.md) — the owner pays; caps; sizes
- [lineage](docs/lineage.md) — eras and PRs; what the 2021 doc still covers
