# CLI Completions for macos && (bash || zsh)
`lockbook` ships with a think completion file for `zsh`, `bash` & `fish`. The [CLI](https://github.com/lockbook/lockbook/blob/master/clients/cli/src/main.rs) performs static and dynamic completions, powered by [`cli-rs`](https://crates.io/crates/cli-rs). You can learn more about the lockbook CLI and its design in [this blog post](https://parth.cafe/p/creating-a-sick-cli).

## Debugging CLI Completions
If `lockbook` completions are not working automatically for you, check out [homebrew's](https://docs.brew.sh/Shell-Completion) guide.

## Manual Creation
If building from source or your package manager doesn't support completions, `lockbook` supports manual creation
### bash
#### Auto-loaded
```
lockbook completions bash >> ~/.bash_completion
```
#### Lazy-loaded
```
lockbook completions bash > ${XDG_DATA_HOME:-~/.local/share}/bash-completion/completions/lockbook
```
### fish
```
lockbook completions fish > ~/.config/fish/completions/lockbook.fish
```
### zsh
- `oh-my-zsh` note: ensure you modify your `$FPATH` before `source $ZSH/oh-my-zsh.sh` because it will call `compinit` for you.
```
lockbook completions zsh > /usr/local/share/zsh/site-functions/_lockbook
```
