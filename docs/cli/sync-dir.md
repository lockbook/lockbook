# sync-dir

Bidirectionally sync a local directory with a lockbook folder.

```
lockbook sync-dir <lockbook-folder> <local-dir> [options]
```

## Options

| Flag | Description |
|------|-------------|
| `--once` | Run a single sync cycle and exit |
| `--no-watch` | Disable filesystem watcher, use polling only |
| `--pull-interval <duration>` | Remote poll interval (default: `5s`). Accepts `5s`, `500ms`, `1m` |

## How it works

Each sync cycle:

1. Scan the local directory and compute SHA-256 content hashes
2. Diff against the last-agreed state (`.sync-dir-state`) to find local new/modified/deleted files
3. Push local changes to lockbook via `create_at_path` + `write_document`
4. `lb.sync()` with the lockbook server
5. Pull remote changes to disk, with conflict detection
6. Save the new agreed state

In long-running mode (the default), cycles are triggered by filesystem events (via `notify`) or the poll interval, whichever comes first. Use `--once` for scripting or one-shot backups.

## Ignore patterns

A `.lockbookignore` file is generated on first run with sensible defaults (`.git/`, `node_modules/`, `target/`, `__pycache__/`, `*.sqlite*`, `.DS_Store`). Add your own patterns using gitignore syntax — the built-in defaults always apply even if you edit the file.

## Conflict handling

When both sides modify the same file between syncs, the local version is saved as `<name>.conflict-<timestamp>.<ext>` and the remote version wins on disk. Both versions are preserved.

## Examples

Sync once and exit:
```bash
lockbook sync-dir my-notes ~/notes --once
```

Long-running sync with 30s polling:
```bash
lockbook sync-dir my-notes ~/notes --pull-interval 30s
```

## Use case: syncing agent config directories

`sync-dir` can keep a server-side directory in sync with your lockbook, making those files browsable and editable from any lockbook client (iOS, Android, desktop). For an example of this pattern — syncing an AI agent's `~/.openclaw` config directory into a shared lockbook folder — see the [openclaw lockbook skill](https://github.com/CoreyCole/lockbook-openclaw-skill).
