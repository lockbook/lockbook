# CLI
The Lockbook CLI is a terminal client for your encrypted files: edit notes, list and search the tree, sync with the server, and copy files between Lockbook and ordinary OS paths when you need plain files outside the app.

Install via a [package manager](installing.md) or [build from source](building.md). Flags and arguments for any command are authoritative in the binary:
```
lockbook --help
lockbook <command> --help
```

This page is a tour and task guide. Prefer `--help` when you need every option.

## Quick start
**New account**
```
lockbook account new <username>
lockbook sync
lockbook list
```

**Existing account** (account key or phrase on stdin, clipboard, or interactive prompt):
```
lockbook account import
lockbook sync
```

**Edit a note**
```
lockbook edit work/todo.md
```

If the path does not exist yet, create it first with `lockbook new work/todo.md`, or pipe content with `lockbook write` (see below).

**Who am I / status**
```
lockbook debug whoami
lockbook account status
```

## Concepts
Files live in a rooted tree (`/…`). Most commands take a **path** or a **file ID** (full UUID or a unique prefix as shown by `lockbook list -l`). Completions expand paths from your local Lockbook data.

Trailing `/` on a path usually means “folder” when creating (`lockbook new notes/` makes a folder).

The CLI keeps an encrypted local store. Edits apply locally first; `lockbook sync` pushes and pulls with the server. `lockbook account status` shows last sync time, unsynced file count, plan, and storage usage.

Point at a non-default server when creating or importing an account with `API_URL` or `--api_url` (see [Configuration](#configuration)).

## Common tasks
List, edit, print, and write notes. `lockbook write` creates the path if it does not exist; sync when you want the server updated:
```
lockbook list
lockbook list -l /work
lockbook edit /work/todo.md
lockbook cat /work/todo.md
echo "hi" | lockbook write /work/todo.md
lockbook sync
```

Copy between Lockbook and ordinary OS paths. `--contents` puts a folder’s children in dest (not `dest/<folder>/`):
```
lockbook import ./photo.png /media/
lockbook export /media/photo.png ./photo.png
lockbook export /notes ./backup --contents
```

Accept a share into your tree (same idea as Shared with me → Files in the apps):
```
lockbook share pending
lockbook share accept <id> /work/
```

Automation examples: [this site’s update script](https://github.com/lockbook/lockbook/blob/master/docs/update.sh), [another blog](https://github.com/Parth/parth.cafe/tree/master/.github/workflows).

## Configuration
| Variable / setting | Purpose |
|--------------------|---------|
| `LOCKBOOK_PATH` | Directory for local state (default under `~/.lockbook/…`) |
| `API_URL` | Server URL when creating or importing an account (also `--api_url` on those commands) |
| `LOCKBOOK_EDITOR` | Editor for `lockbook edit` (else `$EDITOR` / `$VISUAL`, then a platform default) |
| `--editor` on `lockbook edit` | One-shot editor override |

Supported editors for `lockbook edit` include vim, nvim, emacs, helix, nano, sublime, and code; others fall back to the platform default. Inspect the active server and data path with `lockbook debug whereami`.

## Completions
Tab completion covers subcommands and **dynamic paths/IDs** from your Lockbook. Install via your [package manager](installing.md) when possible. Design notes: [Creating a sick CLI](https://lockbook.net/blog/creating-a-sick-cli/). If shell completion is broken system-wide, see [Homebrew’s completion guide](https://docs.brew.sh/Shell-Completion).

Generate scripts manually:
```
# bash (lazy-loaded)
lockbook completions bash > ${XDG_DATA_HOME:-~/.local/share}/bash-completion/completions/lockbook

# fish
lockbook completions fish > ~/.config/fish/completions/lockbook.fish

# zsh (ensure this directory is on $fpath before compinit; with oh-my-zsh, adjust $FPATH before sourcing oh-my-zsh)
lockbook completions zsh > /usr/local/share/zsh/site-functions/_lockbook
```

## Command index
Top-level commands. Nested groups list their subcommands. Details: `lockbook <command> --help`.

| Command | Summary |
|---------|---------|
| `account new` | Create an account |
| `account import` | Import an account from a key or phrase |
| `account export` | Print account key or account phrase (`--phrase`) |
| `account status` | Usage, plan, sync info |
| `account subscribe` / `unsubscribe` | Billing |
| `list` | List files (`-l`, `-R`/`--recursive`, `-p`/`--paths`) |
| `new` | Create a file or folder at a path |
| `edit` | Open a document in an editor |
| `cat` | Print a document to stdout |
| `write` | Write stdin to a document |
| `move` | Move a file to a new parent |
| `rename` | Rename a file |
| `duplicate` | Duplicate a document in place |
| `delete` | Delete a file |
| `search` | Search paths and contents |
| `sync` | Sync with the server |
| `import` | Import files from disk into Lockbook |
| `export` | Export Lockbook files to disk (`--force`, `--contents`) |
| `share new` / `pending` / `accept` / `delete` | Sharing (accept places a share into your tree) |
| `migrate-from bear` | Import a Bear export |
| `fs` | Mount NFS at `/tmp/lockbook` (experimental) |
| `debug whoami` / `whereami` / `info` / `validate` / `debuginfo` | Diagnostics |
| `completions` | Emit shell completion scripts |
