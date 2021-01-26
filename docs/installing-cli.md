# CLI

## Quick Install

Our binary has no dependencies. Simply:
* `curl -O` the latest release for your operating system
* `tar -xzf` the download
* `sudo cp` the binary anywhere on your `$PATH`

Repeat the process to upgrade to newer versions.

[See our downloads.](https://github.com/lockbook/lockbook/releases)

## Package Managers

[Arch Linux](https://aur.archlinux.org/packages/lockbook): `yay -S lockbook`

[MacOS](https://github.com/lockbook/homebrew-lockbook/blob/master/Formula/lockbook.rb): `brew tap lockbook/lockbook && brew install lockbook`

## From Source

Get the Rust toolchain (`rustup`) and ensure `cargo` is on your path.

```
cd clients/cli
API_URL="http://api.lockbook.app:8000" cargo build --release
```

In the `target/release` folder you'll find the `lockbook` binary. Place it anywhere on your `$PATH`. To upgrade, `git pull origin master` and repeat the process.

## Configuration

Design priorities for CLI are inspired by the [Unix philosophy](https://en.wikipedia.org/wiki/Unix_philosophy). This allows it to be minimal and extensible.

Essentially:

What files need to be synced, can be answered by: `lockbook status`, and how many files need to be synced should be answered by `lockbook status | wc -l`.

[A sample configuration for a zsh user.](https://github.com/Parth/dotfiles/blob/master/zsh/lockbook.sh)
