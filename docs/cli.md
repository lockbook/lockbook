# CLI
See [installing](installing.md) or [building](building.md) to aquire Lockbook's CLI.

To sign in: `lockbook account import`
To edit a file: `lockbook edit work/todo.md`

We've invested in nice completions for navigating the CLI. They document all the subcommands as well as dynamically complete values from within your Lockbook. See below for manual setup or troubleshooting.

## Standard streams
Lockbook supports standard streams to get info in and out of lockbook:

`lockbook stream out work/todo.md` will print the contents of `todo.md` to stdout where it can be chained to other programs (like `grep`).

`lockbook stream in file.md` allows you to stream directly into a file.

## FS replication

`lockbook copy` lets you copy files or folders from your file system into lockbook. 
And `lockbook export` lets you copy files or folders from your lockbook onto your filesystem.

Many of these commands can be invoked by ID, seamlessly, for greater operational stability. 

FS replication can be used to build automatic publishing or backup infrastructure. Here are some examples:
* [this website](https://github.com/lockbook/lockbook/blob/master/docs/update.sh)
* [another blog](https://github.com/Parth/parth.cafe/tree/master/.github/workflows)

## Virtual FS (experimental)

`lockbook fs` to run our experimental NFS implementation, which mounts your Lockbook to `/tmp/lockbook` for transparent file system access to your lockbook.

# CLI Configuration

## Customizing CLI's editor
By default `lockbook` will try to determine an editor based on `$EDITOR` or `$VISUAL`. If you want to select an editor specifically for lockbook, distinct from these values, you can set `$LOCKBOOK_EDITOR`

## CLI Completions for `fish`, `bash`, and `zsh`
Lockbook's CLI is built around a sophisticated tab completion behavior. Install the cli using your favorite [package manager](installing.md) or reach out to us if yours isn't listed there. You can configure our CLI to open your favorite text editor allowing you to rapidly jump to your desired note and edit it quickly.

### Completions troubleshooting
`lockbook` ships with a thin completion file for `zsh`, `bash` & `fish`. The [CLI](https://github.com/lockbook/lockbook/blob/master/clients/cli/src/main.rs) performs static and dynamic completions, powered by [`cli-rs`](https://crates.io/crates/cli-rs). You can learn more about the lockbook CLI and its design in [this blog post](https://parth.cafe/p/creating-a-sick-cli).

If `lockbook` completions are not working automatically for you, check out [homebrew's](https://docs.brew.sh/Shell-Completion) guide.

### Manual Creation
If building from source or your package manager doesn't support completions, `lockbook` supports manual creation
#### bash
##### Auto-loaded
```
lockbook completions bash >> ~/.bash_completion
```
##### Lazy-loaded
```
lockbook completions bash > ${XDG_DATA_HOME:-~/.local/share}/bash-completion/completions/lockbook
```
#### fish
```
lockbook completions fish > ~/.config/fish/completions/lockbook.fish
```
#### zsh
- `oh-my-zsh` note: ensure you modify your `$FPATH` before `source $ZSH/oh-my-zsh.sh` because it will call `compinit` for you.
```
lockbook completions zsh > /usr/local/share/zsh/site-functions/_lockbook
```
