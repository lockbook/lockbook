# CLI Installing

## Package Managers

[Arch Linux](https://aur.archlinux.org/packages/lockbook): `yay -S lockbook`

[MacOS](https://github.com/lockbook/homebrew-lockbook/blob/master/Formula/lockbook.rb): `brew tap lockbook/lockbook && brew install lockbook`

[Snap Store](https://snapcraft.io/lockbook) `snap install lockbook`

### Verify snap package prior to installing it

Snap does not check if a package was built by the publisher. As a result, there needs to be an alternative method
to verify this. We have bundled this check into a single command.

- Run `curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/SmailBarkouch/snap-verify/main/snap-verify.sh | sh -s lockbook fqikNXeTHExTwMsksdo78qofrP5HwVISTlZz2dFDsNeNzY_Z6FEki9w9CWqXoDLU`

## Quick Install

Our binary has no dependencies. Simply:
- `curl -O` the latest release for your operating system
- `tar -xzf` the download
- `sudo cp` the binary anywhere on your `$PATH`

Repeat the process to upgrade to newer versions.

[See our downloads.](https://github.com/lockbook/lockbook/releases)
