# CLI
The Lockbook CLI is a terminal client for your encrypted files: edit notes, list and search the tree, sync with the server, and copy files between Lockbook and ordinary OS paths when you need plain files outside the app.

Install via a [package manager](installing.md) or [build from source](building.md). Flags and arguments for any command are authoritative in the binary:
```sh
lockbook --help
lockbook <command> --help
```

This page is a tour and task guide. Prefer `--help` when you need every option.

## Quick start
**New account**
```sh
lockbook account new <username>
lockbook sync
lockbook list
```

**Existing account** (account key or phrase on stdin, clipboard, or interactive prompt):
```sh
lockbook account import
lockbook sync
```

**Edit a note**
```sh
lockbook edit work/todo.md
```

If the path does not exist yet, create it first with `lockbook new work/todo.md`, or pipe content with `lockbook write` (see below).

**Who am I / status**
```sh
lockbook debug whoami
lockbook account status
```

## Concepts
Files live in a rooted tree (`/…`). Most commands take a **path** or a **file ID** (full UUID or a unique prefix as shown by `lockbook list -l`). Completions expand paths from your local Lockbook data.

Trailing `/` on a path usually means “folder” when creating (`lockbook new notes/` makes a folder).

The CLI keeps an encrypted local store. Edits apply locally first; `lockbook sync` pushes and pulls with the server. `lockbook account status` shows last sync time, unsynced file count, plan, and storage usage.

Point at a non-default server when creating or importing an account with `API_URL` or `--api_url` (see [Configuration](#configuration)).

## Common tasks
List, edit, and sync notes. Tab completions expand paths as you type:
```sh
lockbook list
lockbook list -l /work
lockbook edit /work/todo.md
lockbook cat /work/todo.md
echo "take notes" | lockbook write /work/todo.md
lockbook sync
```

Print a document to stdout, or write stdin into Lockbook. `lockbook write` creates the path if it does not exist. Chain with familiar tools—search notes with `grep`/`rg`, fuzzy-pick a path with `fzf`, or grab local logs into a note:
```sh
lockbook cat /work/todo.md | rg "TODO"
lockbook list -p -R / | fzf | xargs lockbook edit
journalctl -u myapp -n 50 --no-pager | lockbook write /ops/myapp-recent.log
echo "hi" | lockbook write /work/todo.md
echo "more" | lockbook write --append /work/todo.md
```

Copy between Lockbook and ordinary OS paths. `--contents` puts a folder’s children in dest (not `dest/<folder>/`):
```sh
lockbook import ./photo.png /media/
lockbook export /media/photo.png ./photo.png
lockbook export /notes ./backup --contents
```

Accept a share into your tree (same idea as Shared with me → Files in the apps):
```sh
lockbook share pending
lockbook share accept <id> /work/
```

Automation examples: [this site’s update script](https://github.com/lockbook/lockbook/blob/master/docs/update.sh), [another blog](https://github.com/Parth/parth.cafe/tree/master/.github/workflows).

## Virtual filesystem (experimental)
`lockbook fs` mounts your Lockbook over NFS at `/tmp/lockbook` so ordinary tools can read and write notes as normal files. Experimental—expect rough edges; prefer the CLI commands above when you can.

```sh
lockbook fs
# then: ls /tmp/lockbook, edit files there, etc.
```

## Configuration
| Variable / setting | Purpose |
|--------------------|---------|
| `LOCKBOOK_PATH` | Directory for local state (default under `~/.lockbook/…`) |
| `API_URL` | Server URL when creating or importing an account (also `--api_url` on those commands) |
| `LOCKBOOK_EDITOR` | Editor for `lockbook edit` (else `$EDITOR` / `$VISUAL`, then a platform default) |
| `--editor` on `lockbook edit` | One-shot editor override |

Supported editors for `lockbook edit` include vim, nvim, emacs, helix, nano, sublime, and code; others fall back to the platform default. Inspect the active server and data path with `lockbook debug whereami`.

## Completions
Tab completion is how you should drive the CLI day to day: static completions for every subcommand and flag, plus **dynamic** completions that pull file paths and IDs from your Lockbook. If something feels hard to type, completions are probably not installed correctly—fix them after install.

Install via your [package manager](installing.md) when possible (preferred). Design notes: [Creating a sick CLI](https://lockbook.net/blog/creating-a-sick-cli/). If shell completion is broken system-wide, see [Homebrew’s completion guide](https://docs.brew.sh/Shell-Completion).

Generate scripts manually when your package manager does not ship them:
```sh
# bash (lazy-loaded)
lockbook completions bash > ${XDG_DATA_HOME:-~/.local/share}/bash-completion/completions/lockbook

# fish
lockbook completions fish > ~/.config/fish/completions/lockbook.fish

# zsh (ensure this directory is on $fpath before compinit; with oh-my-zsh, adjust $FPATH before sourcing oh-my-zsh)
lockbook completions zsh > /usr/local/share/zsh/site-functions/_lockbook
```
