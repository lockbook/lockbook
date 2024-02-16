# CLI Completions
`lockbook` ships with a `_lockbook` tab completions file.
In general, your shell scans `$FPATH` for completions.

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

## Build System
[/utils/releaser/src/linux/cli.rs](https://github.com/lockbook/lockbook/blob/master/utils/releaser/src/linux/cli.rs)
```
lockbook completions bash > lockbook_completions.bash
lockbook completions zsh > lockbook_completions.zsh
lockbook completions fish > lockbook_completions.fish

install -Dm644 lockbook_completions.bash "$pkgdir/usr/share/bash-completion/completions/lockbook"
install -Dm644 lockbook_completions.zsh "$pkgdir/usr/share/zsh/site-functions/_lockbook"
install -Dm644 lockbook_completions.fish "$pkgdir/usr/share/fish/vendor_completions.d/lockbook.fish"
```
